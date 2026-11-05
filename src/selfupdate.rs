//! Autoactualización del propio lanzador desde sus releases de GitHub.
//!
//! Según cómo se instaló, se actualiza de una de estas formas:
//!
//! - **Reemplazo en su sitio** (ejecutable suelto, `.tar.gz`, instalador `-setup.exe` por
//!   usuario): se baja el binario nuevo, se verifica y se sustituye el que está corriendo. No
//!   pide permisos, así que se puede hacer sola en segundo plano.
//! - **`.msi`** (Windows, en Archivos de programa): se baja el `.msi` nuevo y se lanza
//!   `msiexec`, que pide elevación; el lanzador se cierra para que pueda sustituirlo y
//!   `msiexec` lo vuelve a abrir al terminar.
//! - **`.deb` / `.rpm`** (Linux, en `/usr/bin`): se instala el paquete nuevo con pkexec.
//!
//! Todo lo descargado se comprueba contra el `SHA256SUMS` del release y, si el lanzador trae
//! clave pública ([`PUBKEY`]), también contra la firma minisign (`.sig`) del archivo.

use crate::github::{self, Asset, Release};
use crate::state::version_cmp;
use crate::{installer, util, verify};
use std::path::{Path, PathBuf};

pub const REPO: &str = "ellaguno/iureapps";

/// Clave pública (minisign, formato de `tauri signer generate`, en base64) con la que se firman
/// los archivos del release. Mientras esté vacía sólo se comprueba `SHA256SUMS`; en cuanto
/// tenga valor, la firma pasa a ser obligatoria.
pub const PUBKEY: &str = "";

/// Cómo se actualiza esta copia del lanzador.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// Sustituir el ejecutable en su sitio.
    Replace,
    Msi,
    Deb,
    Rpm,
}

impl Method {
    /// Los métodos que piden contraseña o elevación no se lanzan solos.
    pub fn is_silent(&self) -> bool {
        *self == Method::Replace
    }
}

/// Resultado de una actualización correcta.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Ya está instalada la versión nueva; hay que reiniciar para usarla.
    Restart { exe: PathBuf, version: String },
    /// `msiexec` está en marcha y vuelve a abrir el lanzador al terminar: hay que salir ya.
    Exit,
}

fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("ruta del ejecutable: {e}"))?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

/// Si se puede escribir en la carpeta del ejecutable (sin permisos de administrador).
fn dir_writable(exe: &Path) -> bool {
    let Some(dir) = exe.parent() else {
        return false;
    };
    let probe = dir.join(format!(".iureapps-escritura-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Averigua cómo se instaló esta copia.
pub fn method() -> Result<Method, String> {
    let exe = current_exe()?;
    if cfg!(target_os = "linux") {
        let p = exe.to_string_lossy();
        if util::which("dpkg-query").is_some() && util::output("dpkg-query", &["-S", &p]).is_some()
        {
            return Ok(Method::Deb);
        }
        if util::which("rpm").is_some() && util::output("rpm", &["-qf", &p]).is_some() {
            return Ok(Method::Rpm);
        }
    }
    if dir_writable(&exe) {
        return Ok(Method::Replace);
    }
    if cfg!(windows) {
        return Ok(Method::Msi);
    }
    Err(format!(
        "no se puede escribir en {}; descarga la versión nueva a mano",
        exe.parent().unwrap_or(&exe).display()
    ))
}

/// Nombre del archivo del release que corresponde a este equipo y a `method`.
fn wanted(method: &Method, name: &str) -> bool {
    let arch = std::env::consts::ARCH;
    match method {
        Method::Msi => name == "iureapps-windows-x64.msi",
        Method::Deb => name.ends_with(".deb") && (arch != "x86_64" || name.contains("amd64")),
        Method::Rpm => name.ends_with(".rpm") && name.contains(arch),
        Method::Replace => {
            let expected = match (std::env::consts::OS, arch) {
                ("windows", "x86_64") => "iureapps-windows-x64.exe",
                ("linux", "x86_64") => "iureapps-linux-x64.tar.gz",
                ("macos", "aarch64") => "iureapps-macos-arm64.tar.gz",
                ("macos", "x86_64") => "iureapps-macos-x64.tar.gz",
                _ => return false,
            };
            name == expected
        }
    }
}

/// Si `release` es más nueva que la versión en marcha.
pub fn is_newer(release: &Release) -> bool {
    version_cmp(release.version(), env!("CARGO_PKG_VERSION")).is_gt()
}

/// Comprueba `file` contra `SHA256SUMS` y, si hay clave, contra su firma.
fn check(release: &Release, asset: &Asset, file: &Path) -> Result<(), String> {
    let find = |n: &str| release.assets.iter().find(|a| a.name == n);
    let sums = find("SHA256SUMS")
        .ok_or("el release no publica SHA256SUMS; no se puede verificar la descarga")?;
    let sums = github::fetch_text(sums, None)?;
    verify::verify_sha256(&sums, &asset.name, file)?;
    if !PUBKEY.is_empty() {
        let sig = find(&format!("{}.sig", asset.name))
            .ok_or_else(|| format!("el release no trae la firma de {}", asset.name))?;
        let sig = github::fetch_text(sig, None)?;
        verify::verify_file(PUBKEY, &sig, file)?;
    }
    Ok(())
}

/// Descarga, verifica e instala `release`.
pub fn update(
    release: &Release,
    log: &dyn Fn(String),
    progress: &dyn Fn(f32),
) -> Result<Outcome, String> {
    let method = method()?;
    let exe = current_exe()?;
    let asset = release
        .assets
        .iter()
        .find(|a| wanted(&method, &a.name))
        .ok_or_else(|| {
            format!(
                "el release {} no trae archivo para este equipo",
                release.tag_name
            )
        })?;
    let dir = util::data_dir().join("downloads").join("iureapps");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(&asset.name);
    log(format!("descargando {}", asset.name));
    github::download(asset, None, &file, progress)?;
    let result = (|| {
        check(release, asset, &file)?;
        log("descarga verificada".into());
        match method {
            Method::Replace => {
                replace(&file, &dir)?;
                windows_display_version(release.version());
            }
            Method::Deb => installer::install_deb(&file)?,
            Method::Rpm => installer::install_rpm(&file)?,
            Method::Msi => {
                run_msi(&file, &exe)?;
                return Ok(Outcome::Exit);
            }
        }
        Ok(Outcome::Restart {
            exe: exe.clone(),
            version: release.version().to_string(),
        })
    })();
    // El .msi lo sigue usando msiexec; lo demás ya no hace falta.
    if !matches!(result, Ok(Outcome::Exit)) {
        let _ = std::fs::remove_file(&file);
    }
    result
}

/// Sustituye el ejecutable en marcha por el de `file` (`.exe` o `.tar.gz` con `iureapps`).
fn replace(file: &Path, dir: &Path) -> Result<(), String> {
    let new = if file.extension().is_some_and(|e| e == "gz") {
        let tmp = dir.join("extraido");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        util::run(
            "tar",
            &[
                "-xzf",
                &file.to_string_lossy(),
                "-C",
                &tmp.to_string_lossy(),
            ],
        )?;
        tmp.join("iureapps")
    } else {
        file.to_path_buf()
    };
    if !new.is_file() {
        return Err("el archivo descargado no contiene el ejecutable".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    let r = self_replace::self_replace(&new).map_err(|e| format!("no se pudo reemplazar: {e}"));
    let _ = std::fs::remove_dir_all(dir.join("extraido"));
    r
}

/// Con el instalador `-setup.exe` (por usuario) la versión que muestra «Aplicaciones
/// instaladas» se guarda en el registro: se pone al día tras reemplazar el ejecutable.
fn windows_display_version(version: &str) {
    if !cfg!(windows) {
        return;
    }
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\iureapps";
    if util::output("reg", &["query", key, "/v", "DisplayVersion"]).is_some() {
        let _ = util::run(
            "reg",
            &["add", key, "/v", "DisplayVersion", "/d", version, "/f"],
        );
    }
}

/// Lanza `msiexec` desde un script aparte para que siga cuando el lanzador se cierre (espera
/// un par de segundos a que lo haga, para no encontrar el ejecutable en uso) y abre el
/// lanzador al terminar.
fn run_msi(msi: &Path, exe: &Path) -> Result<(), String> {
    let script = msi.with_extension("cmd");
    let text = format!(
        "@echo off\r\nping -n 3 127.0.0.1 >nul\r\nmsiexec /i \"{}\" /passive /norestart\r\nstart \"\" \"{}\"\r\ndel \"{}\"\r\n",
        msi.display(),
        exe.display(),
        msi.display()
    );
    std::fs::write(&script, text).map_err(|e| format!("{}: {e}", script.display()))?;
    util::command("cmd")
        .arg("/C")
        .arg(&script)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("no se pudo lanzar msiexec: {e}"))
}

/// Abre la versión recién instalada con los mismos argumentos y sale.
pub fn restart(exe: &Path) -> ! {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match util::command(&exe.to_string_lossy()).args(&args).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("no se pudo reiniciar {}: {e}", exe.display());
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_assets() {
        assert!(wanted(&Method::Msi, "iureapps-windows-x64.msi"));
        assert!(!wanted(&Method::Msi, "iureapps-windows-x64-setup.exe"));
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert!(wanted(&Method::Replace, "iureapps-linux-x64.tar.gz"));
            assert!(wanted(&Method::Deb, "iureapps_0.1.9-1_amd64.deb"));
            assert!(wanted(&Method::Rpm, "iureapps-0.1.9-1.x86_64.rpm"));
            assert!(!wanted(&Method::Replace, "iureapps-linux-x64.tar.gz.sig"));
        }
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert!(wanted(&Method::Replace, "iureapps-windows-x64.exe"));
            assert!(!wanted(&Method::Replace, "iureapps-windows-x64-setup.exe"));
        }
    }
}
