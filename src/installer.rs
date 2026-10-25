//! Detectar, instalar, abrir y desinstalar apps en Linux, Windows y macOS.

use crate::catalog::{AppDef, Format, Variant};
use crate::github::{self, Release};
use crate::state::Managed;
use crate::system::{Os, SystemInfo};
use crate::util::{self, command, output, run, which};
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Clone, Debug)]
pub enum Launch {
    Exec(PathBuf),
    AppImage(PathBuf),
    MacApp(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Installed {
    pub version: Option<String>,
    pub format: Format,
    pub variant: Option<Variant>,
    pub launch: Launch,
    /// Nombre del paquete (.deb/.rpm) o comando de desinstalación (Windows).
    pub uninstall: Option<String>,
}

/// ¿Está instalada? Primero lo que instaló el lanzador, luego lo que hay en el sistema.
pub fn detect(app: &AppDef, sys: &SystemInfo, managed: Option<&Managed>) -> Option<Installed> {
    if let Some(m) = managed {
        if let Some(p) = m.path.as_ref().filter(|p| p.exists()) {
            let launch = match m.format {
                Format::AppImage => Launch::AppImage(p.clone()),
                Format::MacApp => Launch::MacApp(p.clone()),
                _ => Launch::Exec(p.clone()),
            };
            let version = if m.format == Format::MacApp {
                mac_bundle_version(p).or_else(|| Some(m.version.clone()))
            } else {
                Some(m.version.clone())
            };
            return Some(Installed {
                version,
                format: m.format,
                variant: Some(m.variant),
                launch,
                uninstall: None,
            });
        }
    }
    let mut found = match sys.os {
        Os::Linux => detect_linux(app),
        Os::Windows => detect_windows(app),
        Os::MacOs => detect_mac(app),
        Os::Other => None,
    }?;
    // La variante no se puede saber del sistema: se toma la que instaló el lanzador.
    if found.variant.is_none() {
        found.variant = managed
            .filter(|m| m.format == found.format)
            .map(|m| m.variant);
    }
    Some(found)
}

fn detect_linux(app: &AppDef) -> Option<Installed> {
    for bin in app.linux_bins {
        let path = PathBuf::from("/usr/bin").join(bin);
        if !path.exists() {
            continue;
        }
        let p = path.to_string_lossy().to_string();
        if let Some(pkg) =
            output("dpkg", &["-S", &p]).and_then(|o| o.split(':').next().map(str::to_string))
        {
            let version = output("dpkg-query", &["-W", "-f=${Version}", &pkg]);
            return Some(Installed {
                version,
                format: Format::Deb,
                variant: cuda_hint(app),
                launch: Launch::Exec(path),
                uninstall: Some(pkg),
            });
        }
        if let Some(pkg) = output("rpm", &["-qf", "--qf", "%{NAME}", &p]) {
            let version = output("rpm", &["-q", "--qf", "%{VERSION}", &pkg]);
            return Some(Installed {
                version,
                format: Format::Rpm,
                variant: cuda_hint(app),
                launch: Launch::Exec(path),
                uninstall: Some(pkg),
            });
        }
    }
    None
}

/// El .deb CUDA de IureTranscribe trae el runtime de CUDA en /usr/lib/IureTranscribe.
fn cuda_hint(app: &AppDef) -> Option<Variant> {
    (app.gpu_variants && Path::new(&format!("/usr/lib/{}/libcudart.so.12", app.name)).exists())
        .then_some(Variant::Cuda)
}

fn detect_windows(app: &AppDef) -> Option<Installed> {
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        app.name
    );
    let entry = ["HKCU", "HKLM"]
        .iter()
        .find_map(|hive| reg_query(&format!(r"{hive}\{key}")))?;
    let get = |k: &str| {
        entry
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.clone())
    };
    let exe = get("DisplayIcon")
        .map(|v| {
            v.trim_matches('"')
                .split(',')
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .filter(|v| v.to_lowercase().ends_with(".exe"))
        .or_else(|| {
            get("InstallLocation").map(|d| format!(r"{}\{}.exe", d.trim_matches('"'), app.name))
        })?;
    Some(Installed {
        version: get("DisplayVersion"),
        format: Format::WinExe,
        variant: None,
        launch: Launch::Exec(PathBuf::from(exe)),
        uninstall: get("QuietUninstallString").or_else(|| get("UninstallString")),
    })
}

/// Valores de una clave del registro como pares (nombre, valor).
fn reg_query(key: &str) -> Option<Vec<(String, String)>> {
    let out = output("reg", &["query", key])?;
    let values: Vec<_> = out
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let (name, rest) = l.split_once("    REG_")?;
            let value = rest.split_once("    ").map(|(_, v)| v).unwrap_or("");
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    (!values.is_empty()).then_some(values)
}

fn mac_app_dirs() -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/Applications")];
    if let Some(h) = util::home() {
        v.push(h.join("Applications"));
    }
    v
}

fn detect_mac(app: &AppDef) -> Option<Installed> {
    let path = mac_app_dirs()
        .into_iter()
        .map(|d| d.join(format!("{}.app", app.name)))
        .find(|p| p.exists())?;
    Some(Installed {
        version: mac_bundle_version(&path),
        format: Format::MacApp,
        variant: None,
        launch: Launch::MacApp(path),
        uninstall: None,
    })
}

fn mac_bundle_version(app: &Path) -> Option<String> {
    let plist = std::fs::read_to_string(app.join("Contents/Info.plist")).ok()?;
    let after = plist
        .split("<key>CFBundleShortVersionString</key>")
        .nth(1)?;
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim().to_string())
}

/// Abre la app sin esperar a que termine.
pub fn launch(installed: &Installed, sys: &SystemInfo) -> Result<(), String> {
    let mut cmd = match &installed.launch {
        Launch::Exec(p) => command(&p.to_string_lossy()),
        Launch::AppImage(p) => {
            let mut c = command(&p.to_string_lossy());
            if !sys.fuse {
                // Sin libfuse2 el AppImage se extrae a un directorio temporal y se ejecuta.
                c.env("APPIMAGE_EXTRACT_AND_RUN", "1");
            }
            c
        }
        Launch::MacApp(p) => {
            let mut c = command("open");
            c.arg(p);
            c
        }
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("no se pudo abrir: {e}"))
}

/// Descarga, verifica e instala. `log` recibe mensajes; `progress` el avance de la descarga.
#[allow(clippy::too_many_arguments)]
pub fn install(
    app: &AppDef,
    release: &Release,
    sys: &SystemInfo,
    variant: Variant,
    format: Format,
    token: Option<&str>,
    previous: Option<&Managed>,
    log: &dyn Fn(String),
    progress: &dyn Fn(f32),
) -> Result<Managed, String> {
    let (asset, sig) = app
        .select_asset(release, sys, variant, format)
        .ok_or_else(|| {
            format!(
                "{} {} no tiene {} {} para este equipo",
                app.name,
                release.version(),
                variant.label(),
                format.label()
            )
        })?;

    let downloads = util::data_dir().join("downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let file = downloads.join(&asset.name);
    if asset.size > 0 {
        log(format!(
            "Descargando {} ({})",
            asset.name,
            util::human_size(asset.size)
        ));
    } else {
        log(format!("Descargando {}", asset.name));
    }
    github::download(asset, token, &file, progress)?;

    match (app.pubkey, sig) {
        (Some(key), Some(sig)) => {
            let sig_text = github::fetch_text(sig, token)?;
            if let Err(e) = crate::verify::verify_file(key, &sig_text, &file) {
                let _ = std::fs::remove_file(&file);
                return Err(e);
            }
            log("Firma verificada".into());
        }
        (Some(_), None) => {
            log("Aviso: este instalador no publica firma; se instala sin verificar.".into())
        }
        _ => {}
    }

    let result = match format {
        Format::AppImage => install_appimage(app, sys, &file),
        Format::Deb => install_deb(&file).map(|_| None),
        Format::Rpm => install_rpm(&file).map(|_| None),
        Format::WinExe => install_windows(&file).map(|_| None),
        Format::MacApp => install_mac(app, &file).map(Some),
    };
    let _ = std::fs::remove_file(&file);
    let path = result?;

    // Si cambió de formato (p. ej. de AppImage a .deb para usar CUDA) se quita el anterior.
    if let Some(prev) = previous {
        if prev.format == Format::AppImage && format != Format::AppImage {
            if let Some(p) = &prev.path {
                let _ = std::fs::remove_file(p);
                let _ = std::fs::remove_file(desktop_file(app));
                log("Se quitó el AppImage anterior.".into());
            }
        }
    }
    Ok(Managed {
        version: release.version().to_string(),
        variant,
        format,
        path,
    })
}

fn install_appimage(
    app: &AppDef,
    sys: &SystemInfo,
    file: &Path,
) -> Result<Option<PathBuf>, String> {
    let dir = util::data_dir().join("apps");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{}.AppImage", app.name));
    std::fs::copy(file, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    // Entrada en el menú de aplicaciones.
    let exec = if sys.fuse {
        format!("\"{}\" %F", dest.display())
    } else {
        format!("env APPIMAGE_EXTRACT_AND_RUN=1 \"{}\" %F", dest.display())
    };
    let desktop = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={exec}\nIcon=application-x-executable\nTerminal=false\nCategories=Office;\nX-Iureapps=true\n",
        app.name, app.description
    );
    let df = desktop_file(app);
    if let Some(d) = df.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(df, desktop);
    Ok(Some(dest))
}

fn desktop_file(app: &AppDef) -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| util::home().map(|h| h.join(".local/share")))
        .unwrap_or_default();
    base.join("applications")
        .join(format!("iureapps-{}.desktop", app.id))
}

/// Ejecuta un comando como administrador con pkexec (diálogo gráfico de contraseña).
fn as_root(args: &[&str]) -> Result<(), String> {
    if which("pkexec").is_none() {
        return Err(format!(
            "hace falta permiso de administrador; ejecuta en una terminal:\n  sudo {}",
            args.join(" ")
        ));
    }
    run("pkexec", args)
}

fn install_deb(file: &Path) -> Result<(), String> {
    let f = file.to_string_lossy();
    if which("apt-get").is_some() {
        as_root(&["apt-get", "install", "-y", "--allow-downgrades", &f])
    } else {
        as_root(&["dpkg", "-i", &f])
    }
}

fn install_rpm(file: &Path) -> Result<(), String> {
    let f = file.to_string_lossy();
    if which("dnf").is_some() {
        as_root(&["dnf", "install", "-y", "--nogpgcheck", &f])
    } else if which("zypper").is_some() {
        as_root(&[
            "zypper",
            "--non-interactive",
            "install",
            "--allow-unsigned-rpm",
            &f,
        ])
    } else {
        as_root(&["rpm", "-Uvh", "--replacepkgs", &f])
    }
}

/// Start-Process pide elevación (UAC) cuando el instalador la necesita.
fn ps_run_wait(exe: &str, args: &str) -> Result<(), String> {
    let exe = exe.replace('\'', "''");
    let script = if args.is_empty() {
        format!("$p = Start-Process -FilePath '{exe}' -Wait -PassThru; exit $p.ExitCode")
    } else {
        format!("$p = Start-Process -FilePath '{exe}' -ArgumentList '{args}' -Wait -PassThru; exit $p.ExitCode")
    };
    run("powershell", &["-NoProfile", "-Command", &script])
}

fn install_windows(file: &Path) -> Result<(), String> {
    ps_run_wait(&file.to_string_lossy(), "/S")
}

fn install_mac(app: &AppDef, file: &Path) -> Result<PathBuf, String> {
    let tmp = util::data_dir().join("tmp").join(app.id);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    run(
        "tar",
        &[
            "-xzf",
            &file.to_string_lossy(),
            "-C",
            &tmp.to_string_lossy(),
        ],
    )?;
    let bundle = std::fs::read_dir(&tmp)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .ok_or("el archivo no contiene una .app")?;
    // /Applications si se puede escribir; si no, ~/Applications.
    let dir = mac_app_dirs()
        .into_iter()
        .find(|d| {
            let _ = std::fs::create_dir_all(d);
            let probe = d.join(".iureapps-probe");
            let ok = std::fs::write(&probe, b"").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        })
        .ok_or("no hay carpeta de Aplicaciones con permiso de escritura")?;
    let dest = dir.join(format!("{}.app", app.name));
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("no se pudo reemplazar {}: {e}", dest.display()))?;
    }
    if std::fs::rename(&bundle, &dest).is_err() {
        run("mv", &[&bundle.to_string_lossy(), &dest.to_string_lossy()])?;
    }
    let _ = run(
        "xattr",
        &["-dr", "com.apple.quarantine", &dest.to_string_lossy()],
    );
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(dest)
}

pub fn uninstall(app: &AppDef, installed: &Installed) -> Result<(), String> {
    match (installed.format, &installed.launch) {
        (Format::AppImage, Launch::AppImage(p)) => {
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(desktop_file(app));
            Ok(())
        }
        (Format::MacApp, Launch::MacApp(p)) => {
            std::fs::remove_dir_all(p).map_err(|e| e.to_string())
        }
        (Format::Deb, _) => {
            let pkg = installed
                .uninstall
                .as_deref()
                .ok_or("paquete desconocido")?;
            as_root(&["apt-get", "remove", "-y", pkg])
        }
        (Format::Rpm, _) => {
            let pkg = installed
                .uninstall
                .as_deref()
                .ok_or("paquete desconocido")?;
            if which("dnf").is_some() {
                as_root(&["dnf", "remove", "-y", pkg])
            } else {
                as_root(&["rpm", "-e", pkg])
            }
        }
        (Format::WinExe, _) => {
            let cmd = installed
                .uninstall
                .as_deref()
                .ok_or("no se encontró el desinstalador")?;
            // UninstallString suele ser "C:\...\uninstall.exe" (con comillas).
            let exe = if let Some(rest) = cmd.strip_prefix('"') {
                rest.split('"').next().unwrap_or(rest)
            } else {
                cmd.split(" /").next().unwrap_or(cmd)
            };
            ps_run_wait(exe, "/S")
        }
        _ => Err("no se sabe desinstalar este formato".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Asset;

    /// Descarga real desde GitHub: `cargo test -- --ignored installs_real_appimage`.
    #[test]
    #[ignore]
    fn installs_real_appimage() {
        let ver = std::env::var("IUREDITOR_VERSION").unwrap_or_else(|_| "1.13.0".into());
        let base = format!("https://github.com/ellaguno/iureditor/releases/download/v{ver}/");
        let asset = |name: String| Asset {
            browser_download_url: format!("{base}{name}"),
            url: String::new(),
            size: 0,
            name,
        };
        let name = format!("iureditor_{ver}_3-linux-x64.AppImage");
        let release = Release {
            tag_name: format!("v{ver}"),
            html_url: String::new(),
            body: None,
            published_at: None,
            assets: vec![asset(name.clone()), asset(format!("{name}.sig"))],
        };
        let app = crate::catalog::find("iureditor").unwrap();
        let sys = SystemInfo::detect();
        let m = install(
            app,
            &release,
            &sys,
            Variant::Cpu,
            Format::AppImage,
            None,
            None,
            &|m| println!("{m}"),
            &|_| {},
        )
        .unwrap();
        let path = m.path.unwrap();
        assert!(path.exists());
        let inst = detect(
            app,
            &sys,
            Some(&Managed {
                version: ver.clone(),
                variant: Variant::Cpu,
                format: Format::AppImage,
                path: Some(path.clone()),
            }),
        )
        .unwrap();
        assert_eq!(inst.version.as_deref(), Some(ver.as_str()));
        uninstall(app, &inst).unwrap();
        assert!(!path.exists());
    }
}
