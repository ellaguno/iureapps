//! Actualización del propio lanzador: baja de su release el archivo que corresponde a como está
//! instalado, comprueba su firma minisign, se reemplaza y se vuelve a abrir.
//!
//! Las releases de iureapps se firman con la misma clave del actualizador de Tauri que
//! iureditor, IureTranscribe e IureDav (ver `.github/workflows`).

use crate::catalog::KEY_8F1B;
use crate::github::{self, Release};
use crate::system::{Arch, Os, PkgFamily, SystemInfo};
use crate::{installer, util, verify};
use std::path::{Path, PathBuf};

/// Repo de las releases del lanzador.
pub const REPO: &str = "ellaguno/iureapps";

/// Clave pública con la que se firman las releases de iureapps.
const PUBKEY: &str = KEY_8F1B;

/// Variable que lleva la instancia recién actualizada: no debe ceder ante la anterior, que
/// todavía se está cerrando.
pub const RESTARTED_ENV: &str = "IUREAPPS_REINICIADO";

/// Cómo está instalado este ejecutable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    /// Paquete del sistema (`/usr/bin/iureapps`): se instala el paquete nuevo con pkexec.
    Deb,
    Rpm,
    /// Ejecutable suelto: se reemplaza el archivo.
    Binary(PathBuf),
    /// `iureapps.app` de macOS: se reemplaza el bundle con la del `.dmg`.
    MacApp(PathBuf),
}

/// Qué abrir después de actualizar.
#[derive(Clone, Debug)]
pub enum Relaunch {
    Exe(PathBuf),
    MacApp(PathBuf),
}

/// Cómo se instaló este ejecutable, según dónde está.
pub fn method(sys: &SystemInfo) -> Result<Method, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.canonicalize().unwrap_or(exe);
    Ok(method_for(&exe, sys.os, sys.pkg))
}

fn method_for(exe: &Path, os: Os, pkg: PkgFamily) -> Method {
    match os {
        Os::Linux if exe.starts_with("/usr") => match pkg {
            PkgFamily::Rpm => Method::Rpm,
            // Sin dpkg ni rpm reconocibles, como mejor apuesta, el .deb (falla con un error claro).
            _ => Method::Deb,
        },
        Os::MacOs => {
            // …/iureapps.app/Contents/MacOS/iureapps
            let bundle = exe.parent().and_then(Path::parent).and_then(Path::parent);
            match bundle {
                Some(b) if b.extension().is_some_and(|x| x == "app") => {
                    Method::MacApp(b.to_path_buf())
                }
                _ => Method::Binary(exe.to_path_buf()),
            }
        }
        _ => Method::Binary(exe.to_path_buf()),
    }
}

/// Nombre del archivo de la release: exacto, o con principio y final (los paquetes llevan
/// la versión en medio).
#[derive(Clone, Copy)]
enum Pattern {
    Exact(&'static str),
    Affix(&'static str, &'static str),
}

impl Pattern {
    fn matches(self, name: &str) -> bool {
        match self {
            Pattern::Exact(n) => name == n,
            Pattern::Affix(a, b) => name.starts_with(a) && name.ends_with(b),
        }
    }
}

/// El archivo de la release para este método y equipo.
fn wanted(method: &Method, os: Os, arch: Arch) -> Option<Pattern> {
    use Pattern::{Affix, Exact};
    Some(match (method, os, arch) {
        (Method::Deb, _, Arch::X64) => Affix("iureapps_", "_amd64.deb"),
        (Method::Rpm, _, Arch::X64) => Affix("iureapps-", ".x86_64.rpm"),
        (Method::MacApp(_), _, _) => Exact("iureapps-macos-universal.dmg"),
        (Method::Binary(_), Os::Linux, Arch::X64) => Exact("iureapps-linux-x64.tar.gz"),
        (Method::Binary(_), Os::Windows, Arch::X64) => Exact("iureapps-windows-x64.exe"),
        (Method::Binary(_), Os::MacOs, Arch::Arm64) => Exact("iureapps-macos-arm64.tar.gz"),
        (Method::Binary(_), Os::MacOs, Arch::X64) => Exact("iureapps-macos-x64.tar.gz"),
        _ => return None,
    })
}

/// Lo que se va a hacer: el archivo y su firma, o por qué no se puede actualizar solo.
pub struct Plan {
    pub method: Method,
    pub asset: github::Asset,
    pub sig: github::Asset,
}

pub fn plan(release: &Release, sys: &SystemInfo) -> Result<Plan, String> {
    let method = method(sys)?;
    let pick = wanted(&method, sys.os, sys.arch)
        .ok_or("no hay una versión del lanzador para este equipo")?;
    let asset = release
        .assets
        .iter()
        .find(|a| pick.matches(&a.name))
        .cloned()
        .ok_or_else(|| {
            format!(
                "la versión {} no trae el archivo para este equipo",
                release.version()
            )
        })?;
    let sig_name = format!("{}.sig", asset.name);
    let sig = release
        .assets
        .iter()
        .find(|a| a.name == sig_name)
        .cloned()
        .ok_or_else(|| format!("la versión {} no está firmada", release.version()))?;
    Ok(Plan { method, asset, sig })
}

/// Descarga, verifica e instala la versión de `plan`. Devuelve qué abrir para reiniciar.
pub fn apply(
    plan: &Plan,
    token: Option<&str>,
    log: &dyn Fn(String),
    progress: &dyn Fn(f32),
) -> Result<Relaunch, String> {
    let dir = util::data_dir().join("tmp").join("iureapps-update");
    apply_with(plan, PUBKEY, token, &dir, log, progress)
}

fn apply_with(
    plan: &Plan,
    pubkey: &str,
    token: Option<&str>,
    dir: &Path,
    log: &dyn Fn(String),
    progress: &dyn Fn(f32),
) -> Result<Relaunch, String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(&plan.asset.name);
    log(format!("descargando {}", plan.asset.name));
    let result = (|| {
        github::download(&plan.asset, token, &file, |p| progress(p * 0.9))?;
        let sig = github::fetch_text(&plan.sig, token)?;
        verify::verify_file(pubkey, &sig, &file)?;
        log("firma verificada".into());
        progress(0.92);
        let relaunch = install(&plan.method, &file, dir)?;
        progress(1.0);
        Ok(relaunch)
    })();
    let _ = std::fs::remove_dir_all(dir);
    result
}

fn install(method: &Method, file: &Path, dir: &Path) -> Result<Relaunch, String> {
    match method {
        Method::Deb => {
            installer::install_deb(file)?;
            Ok(Relaunch::Exe(PathBuf::from("/usr/bin/iureapps")))
        }
        Method::Rpm => {
            installer::install_rpm(file)?;
            Ok(Relaunch::Exe(PathBuf::from("/usr/bin/iureapps")))
        }
        Method::MacApp(bundle) => {
            installer::install_mac_dmg(file, dir, bundle)?;
            let _ = util::run(
                "xattr",
                &["-dr", "com.apple.quarantine", &bundle.to_string_lossy()],
            );
            Ok(Relaunch::MacApp(bundle.clone()))
        }
        Method::Binary(exe) => {
            let new = if file.extension().is_some_and(|x| x == "gz") {
                let out = dir.join("x");
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                util::run(
                    "tar",
                    &[
                        "-xzf",
                        &file.to_string_lossy(),
                        "-C",
                        &out.to_string_lossy(),
                    ],
                )?;
                let bin = out.join("iureapps");
                if !bin.is_file() {
                    return Err("el archivo no contiene el ejecutable iureapps".into());
                }
                bin
            } else {
                file.to_path_buf()
            };
            replace_binary(exe, &new)?;
            Ok(Relaunch::Exe(exe.clone()))
        }
    }
}

/// Copia del ejecutable anterior que deja Windows (no se puede borrar mientras corre).
fn old_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    exe.with_file_name(name)
}

/// Pone `new` en lugar de `exe`, aunque `exe` esté corriendo. Primero se copia junto a `exe`
/// (mismo sistema de archivos) y luego se renombra, para no dejar nunca un ejecutable a medias.
fn replace_binary(exe: &Path, new: &Path) -> Result<(), String> {
    let dir = exe.parent().ok_or("ruta del ejecutable sin carpeta")?;
    let staged = dir.join(".iureapps.nuevo");
    let denied = |e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "no hay permiso para escribir en {}; descarga la versión nueva a mano",
                dir.display()
            )
        } else {
            format!("{}: {e}", dir.display())
        }
    };
    std::fs::copy(new, &staged).map_err(denied)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
    }
    // Windows no deja sobrescribir un .exe abierto, pero sí renombrarlo.
    let old = old_path(exe);
    if cfg!(windows) {
        let _ = std::fs::remove_file(&old);
        if let Err(e) = std::fs::rename(exe, &old) {
            let _ = std::fs::remove_file(&staged);
            return Err(denied(e));
        }
    }
    if let Err(e) = std::fs::rename(&staged, exe) {
        if cfg!(windows) {
            let _ = std::fs::rename(&old, exe);
        }
        let _ = std::fs::remove_file(&staged);
        return Err(denied(e));
    }
    Ok(())
}

/// Borra el ejecutable anterior que dejó una actualización en Windows.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
    }
}

/// Abre la versión nueva (que no cede ante esta instancia, que va a cerrarse).
pub fn relaunch(target: &Relaunch) -> Result<(), String> {
    let mut cmd = match target {
        Relaunch::Exe(exe) => util::command(&exe.to_string_lossy()),
        Relaunch::MacApp(bundle) => {
            let mut c = util::command("open");
            c.arg("-n").arg(bundle);
            c
        }
    };
    cmd.env(RESTARTED_ENV, "1")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("no se pudo abrir la versión nueva: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(m: &Method, os: Os, arch: Arch, names: &[&str]) -> Option<String> {
        let f = wanted(m, os, arch)?;
        names.iter().find(|n| f.matches(n)).map(|n| n.to_string())
    }

    const ASSETS: &[&str] = &[
        "iureapps-linux-x64.tar.gz",
        "iureapps-windows-x64.exe",
        "iureapps-macos-arm64.tar.gz",
        "iureapps-macos-x64.tar.gz",
        "iureapps-macos-universal.dmg",
        "iureapps_0.1.13-1_amd64.deb",
        "iureapps-0.1.13-1.x86_64.rpm",
    ];

    #[test]
    fn metodo_segun_la_ruta() {
        let deb = method_for(Path::new("/usr/bin/iureapps"), Os::Linux, PkgFamily::Deb);
        assert_eq!(deb, Method::Deb);
        let rpm = method_for(Path::new("/usr/bin/iureapps"), Os::Linux, PkgFamily::Rpm);
        assert_eq!(rpm, Method::Rpm);
        let suelto = Path::new("/home/ana/bin/iureapps");
        assert_eq!(
            method_for(suelto, Os::Linux, PkgFamily::Deb),
            Method::Binary(suelto.into())
        );
        let mac = Path::new("/Applications/iureapps.app/Contents/MacOS/iureapps");
        assert_eq!(
            method_for(mac, Os::MacOs, PkgFamily::Other),
            Method::MacApp("/Applications/iureapps.app".into())
        );
    }

    #[test]
    fn archivo_de_cada_equipo() {
        let bin = Method::Binary("/x/iureapps".into());
        let app = Method::MacApp("/Applications/iureapps.app".into());
        let cases = [
            (
                &Method::Deb,
                Os::Linux,
                Arch::X64,
                "iureapps_0.1.13-1_amd64.deb",
            ),
            (
                &Method::Rpm,
                Os::Linux,
                Arch::X64,
                "iureapps-0.1.13-1.x86_64.rpm",
            ),
            (&bin, Os::Linux, Arch::X64, "iureapps-linux-x64.tar.gz"),
            (&bin, Os::Windows, Arch::X64, "iureapps-windows-x64.exe"),
            (&bin, Os::MacOs, Arch::Arm64, "iureapps-macos-arm64.tar.gz"),
            (&app, Os::MacOs, Arch::Arm64, "iureapps-macos-universal.dmg"),
        ];
        for (m, os, arch, want) in cases {
            assert_eq!(pick(m, os, arch, ASSETS).as_deref(), Some(want), "{m:?}");
        }
        // Sin compilación para Linux arm64.
        assert_eq!(pick(&bin, Os::Linux, Arch::Arm64, ASSETS), None);
    }

    #[test]
    fn reemplaza_el_ejecutable() {
        let dir = std::env::temp_dir().join(format!("iureapps-self-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("iureapps");
        let new = dir.join("nuevo");
        std::fs::write(&exe, b"viejo").unwrap();
        std::fs::write(&new, b"nuevo").unwrap();
        replace_binary(&exe, &new).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"nuevo");
        assert!(!dir.join(".iureapps.nuevo").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Clave y firma de prueba (`tauri signer generate`/`sign`, como en los workflows) de
    /// `NUEVO`, publicado como `iureapps-windows-x64.exe`.
    const TEST_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDQzRjMzNDc0MzM3QkVBNTMKUldSVDZuc3pkRFR6UStpc1BnVG1qOUV6MitJTXY3eW1DVDVBSDd4bkZHbE9XMHd1dVhnNnZmKysK";
    const TEST_SIG: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSVDZuc3pkRFR6UXpQM2dWcSthMDdqTGsvYUU0K1JOVWV4Tit4Z2RzYXMxTjkzMmY5VTR3emsvNXRObmYvZW14bmtROVhZS1lSYjJEUGRleG1EM3IvYVYxMDZ3d1pEaEFjPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0OTM5MDQ1CWZpbGU6aXVyZWFwcHMtd2luZG93cy14NjQuZXhlCm5QZWpyd0tZRkNqcFBMeUgrUzNET0tsUVR5cTBrb0dRb1RKQ3Axdm5DeFgzTVJTaUFkSmpLSzd0c1o2M3BhT3RRTDBHQ2pXRC9abmlDNkw2Um1uVkNBPT0K";
    const NUEVO: &[u8] = b"iureapps nuevo\n";

    /// Servidor HTTP mínimo que sirve `files` (nombre → contenido) a cualquier petición.
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> u16 {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut line = String::new();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let _ = reader.read_line(&mut line);
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) <= 2 {
                        break;
                    }
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/");
                let body = files
                    .iter()
                    .find(|(n, _)| path.ends_with(n))
                    .map(|(_, b)| b.clone());
                let (status, body) = match body {
                    Some(b) => ("200 OK", b),
                    None => ("404 Not Found", Vec::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        port
    }

    fn test_plan(port: u16, exe: &Path) -> Plan {
        let asset = |name: &str| github::Asset {
            name: name.into(),
            size: 0,
            browser_download_url: format!("http://127.0.0.1:{port}/{name}"),
            url: String::new(),
        };
        Plan {
            method: Method::Binary(exe.to_path_buf()),
            asset: asset("iureapps-windows-x64.exe"),
            sig: asset("iureapps-windows-x64.exe.sig"),
        }
    }

    #[test]
    fn descarga_verifica_y_reemplaza() {
        let base = std::env::temp_dir().join(format!("iureapps-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let exe = base.join("iureapps");

        // Firma buena: se reemplaza el ejecutable.
        std::fs::write(&exe, b"viejo").unwrap();
        let port = serve(vec![
            ("iureapps-windows-x64.exe", NUEVO.to_vec()),
            ("iureapps-windows-x64.exe.sig", TEST_SIG.as_bytes().to_vec()),
        ]);
        let r = apply_with(
            &test_plan(port, &exe),
            TEST_PUBKEY,
            None,
            &base.join("tmp"),
            &|_| {},
            &|_| {},
        );
        assert!(matches!(r, Ok(Relaunch::Exe(ref p)) if p == &exe), "{r:?}");
        assert_eq!(std::fs::read(&exe).unwrap(), NUEVO);

        // Archivo alterado: no se toca el ejecutable.
        std::fs::write(&exe, b"viejo").unwrap();
        let port = serve(vec![
            ("iureapps-windows-x64.exe", b"malicioso".to_vec()),
            ("iureapps-windows-x64.exe.sig", TEST_SIG.as_bytes().to_vec()),
        ]);
        let r = apply_with(
            &test_plan(port, &exe),
            TEST_PUBKEY,
            None,
            &base.join("tmp"),
            &|_| {},
            &|_| {},
        );
        assert!(r.is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"viejo");

        // Firmado con otra clave (la real del lanzador): tampoco.
        let r = apply_with(
            &test_plan(
                serve(vec![
                    ("iureapps-windows-x64.exe", NUEVO.to_vec()),
                    ("iureapps-windows-x64.exe.sig", TEST_SIG.as_bytes().to_vec()),
                ]),
                &exe,
            ),
            PUBKEY,
            None,
            &base.join("tmp"),
            &|_| {},
            &|_| {},
        );
        assert!(r.is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"viejo");
        let _ = std::fs::remove_dir_all(&base);
    }
}
