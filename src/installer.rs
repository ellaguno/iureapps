//! Detectar, instalar, abrir y desinstalar apps en Linux, Windows y macOS.

use crate::catalog::{AppDef, Format, Packaging, Variant};
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
    if let Some(found) = detect_suite_msi(app) {
        return Some(found);
    }
    let key = format!(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{}",
        app.name
    );
    // Inno Setup registra la app como `<AppId>_is1`.
    let entry = ["HKCU", "HKLM"]
        .iter()
        .flat_map(|hive| [format!(r"{hive}\{key}"), format!(r"{hive}\{key}_is1")])
        .find_map(|k| reg_query(&k))?;
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

/// Un `.msi` de la suite (IureProject): Windows muestra su `ProductVersion` (`0.2.15`), no la
/// versión, así que el instalador deja la de verdad, la carpeta y el `ProductCode` en
/// `HKLM\Software\Iurefficient\<App>`.
fn detect_suite_msi(app: &AppDef) -> Option<Installed> {
    let entry = reg_query(&format!(r"HKLM\Software\Iurefficient\{}", app.name))?;
    let get = |k: &str| {
        entry
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let code = get("ProductCode").filter(|c| is_guid(c))?;
    let bin = app.linux_bins.first()?;
    let exe = PathBuf::from(get("InstallDir")?).join(format!("{bin}.exe"));
    if !exe.exists() {
        return None;
    }
    Some(Installed {
        version: get("Version"),
        format: Format::Msi,
        variant: None,
        launch: Launch::Exec(exe),
        uninstall: Some(code),
    })
}

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` (lo que acepta `msiexec /x`; viene del registro).
fn is_guid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 38
        && b.first() == Some(&b'{')
        && b.last() == Some(&b'}')
        && b.iter().enumerate().skip(1).take(36).all(|(i, c)| {
            if matches!(i, 9 | 14 | 19 | 24) {
                *c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
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
    mac_bundle_key(app, "CFBundleShortVersionString")
}

/// Valor de texto de una clave del `Info.plist` (en XML) de la `.app`.
fn mac_bundle_key(app: &Path, key: &str) -> Option<String> {
    let plist = std::fs::read_to_string(app.join("Contents/Info.plist")).ok()?;
    let after = plist.split(&format!("<key>{key}</key>")).nth(1)?;
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
        (None, _) => match release.assets.iter().find(|a| a.name == "SHA256SUMS") {
            Some(sums) => {
                let text = github::fetch_text(sums, token)?;
                if let Err(e) = crate::verify::verify_sha256(&text, &asset.name, &file) {
                    let _ = std::fs::remove_file(&file);
                    return Err(e);
                }
                log("Suma SHA-256 verificada".into());
            }
            None => {
                log("Aviso: la release no publica SHA256SUMS; se instala sin verificar.".into())
            }
        },
    }

    let result = match format {
        Format::AppImage => install_appimage(app, sys, &file),
        Format::Deb => install_deb(&file).map(|_| None),
        Format::Rpm => install_rpm(&file).map(|_| None),
        Format::WinExe => install_windows(app, &file).map(|_| None),
        Format::Msi => install_msi(&file).map(|_| None),
        Format::MacApp => install_mac(app, &file).map(Some),
        Format::TarGz => install_tarball(app, &file).map(Some),
    };
    let _ = std::fs::remove_file(&file);
    let path = result?;

    // Si cambió de formato (p. ej. de AppImage a .deb para usar CUDA) se quita el anterior.
    if let Some(prev) = previous.filter(|p| p.format != format) {
        if let Some(p) = &prev.path {
            let removed = match prev.format {
                Format::AppImage => std::fs::remove_file(p).is_ok(),
                Format::TarGz => remove_tarball_dir(app, p),
                _ => false,
            };
            if removed {
                // AppImage y .tar.gz escriben la misma entrada del menú: no borrar la nueva.
                if !matches!(format, Format::AppImage | Format::TarGz) {
                    let _ = std::fs::remove_file(desktop_file(app));
                }
                log(format!(
                    "Se quitó la instalación anterior ({}).",
                    prev.format.label()
                ));
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
    let exec = if sys.fuse {
        format!("\"{}\" %F", dest.display())
    } else {
        format!("env APPIMAGE_EXTRACT_AND_RUN=1 \"{}\" %F", dest.display())
    };
    let meta = appimage_meta(app, &dest);
    write_desktop(app, &exec, &meta);
    Ok(Some(dest))
}

/// Lo que el menú necesita del paquete: icono ya copiado fuera y clase de ventana.
#[derive(Debug, Default, Clone, PartialEq)]
struct DesktopMeta {
    icon: Option<PathBuf>,
    wm_class: Option<String>,
}

/// Entrada en el menú de aplicaciones del usuario. Sin `Icon` ni `StartupWMClass` reales, GNOME
/// muestra un engrane y no asocia la ventana con su entrada.
fn write_desktop(app: &AppDef, exec: &str, meta: &DesktopMeta) {
    let df = desktop_file(app);
    if let Some(d) = df.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(df, desktop_entry(app, exec, meta));
}

fn desktop_entry(app: &AppDef, exec: &str, meta: &DesktopMeta) -> String {
    let icon = meta
        .icon
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "application-x-executable".into());
    let mut out = format!(
        "[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={exec}\nIcon={icon}\nTerminal=false\nCategories=Office;\n",
        app.name, app.description
    );
    if let Some(w) = &meta.wm_class {
        out.push_str(&format!("StartupWMClass={w}\nStartupNotify=true\n"));
    }
    out.push_str("X-Iureapps=true\n");
    out
}

/// Icono y `StartupWMClass` del propio AppImage: `.DirIcon` y el `.desktop` de
/// `usr/share/applications`, sacados con `--appimage-extract` (no necesita FUSE). El icono se
/// copia a `<datos>/icons/<id>.png|svg` para que siga ahí si el AppImage cambia.
fn appimage_meta(app: &AppDef, appimage: &Path) -> DesktopMeta {
    let tmp = util::data_dir()
        .join("tmp")
        .join(format!("meta-{}", app.id));
    let _ = std::fs::remove_dir_all(&tmp);
    if std::fs::create_dir_all(&tmp).is_err() {
        return DesktopMeta::default();
    }
    let extract = |pattern: &str| {
        let _ = util::command(&appimage.to_string_lossy())
            .arg("--appimage-extract")
            .arg(pattern)
            .current_dir(&tmp)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    let root = tmp.join("squashfs-root");
    extract(".DirIcon");
    extract("usr/share/applications/*.desktop");
    // `.DirIcon` suele ser un enlace a un archivo de dentro: se extrae también el destino.
    let mut icon_src = root.join(".DirIcon");
    for _ in 0..4 {
        let Ok(target) = std::fs::read_link(&icon_src) else {
            break;
        };
        let rel = target.to_string_lossy().trim_start_matches('/').to_string();
        if rel.split('/').any(|c| c == "..") {
            break;
        }
        extract(&rel);
        icon_src = root.join(rel);
    }
    let icon = std::fs::read(&icon_src).ok().and_then(|bytes| {
        let ext = icon_ext(&bytes)?;
        let dir = util::data_dir().join("icons");
        std::fs::create_dir_all(&dir).ok()?;
        let dest = dir.join(format!("{}.{ext}", app.id));
        std::fs::write(&dest, &bytes).ok()?;
        Some(dest)
    });
    let wm_class = std::fs::read_dir(root.join("usr/share/applications"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .find_map(|t| desktop_key(&t, "StartupWMClass"));
    let _ = std::fs::remove_dir_all(&tmp);
    DesktopMeta { icon, wm_class }
}

/// Extensión del icono por su contenido (PNG o SVG); otra cosa no se usa.
fn icon_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    let head = String::from_utf8_lossy(bytes.get(..bytes.len().min(512))?);
    (head.contains("<svg")).then_some("svg")
}

/// Valor de una clave de la sección `[Desktop Entry]` (sin las traducciones `Clave[es]`).
fn desktop_key(text: &str, key: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key && !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// Al arrancar: las entradas que escribieron versiones anteriores (engrane, sin `StartupWMClass`)
/// se rehacen con el icono y la clase del AppImage instalado. Rápido si no hay nada que hacer.
pub fn repair_desktop_entries() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for app in crate::catalog::APPS {
        let df = desktop_file(app);
        let Ok(text) = std::fs::read_to_string(&df) else {
            continue;
        };
        if !text.contains("X-Iureapps=true") || !text.contains("Icon=application-x-executable") {
            continue;
        }
        let Some(exec) = desktop_key(&text, "Exec") else {
            continue;
        };
        let appimage = util::data_dir()
            .join("apps")
            .join(format!("{}.AppImage", app.name));
        if !appimage.is_file() || !exec.contains(&*appimage.to_string_lossy()) {
            continue;
        }
        let meta = appimage_meta(app, &appimage);
        if meta != DesktopMeta::default() {
            write_desktop(app, &exec, &meta);
        }
    }
}

/// Carpeta de una app instalada desde `.tar.gz`.
fn tarball_dir(app: &AppDef) -> PathBuf {
    util::data_dir().join("apps").join(app.id)
}

/// Borra la carpeta del `.tar.gz` si `bin` está dentro de la del lanzador (nunca otra cosa).
fn remove_tarball_dir(app: &AppDef, bin: &Path) -> bool {
    let dir = tarball_dir(app);
    bin.starts_with(&dir) && std::fs::remove_dir_all(&dir).is_ok()
}

/// Descomprime los binarios en `~/.local/share/iureapps/apps/<id>/` y devuelve el principal.
fn install_tarball(app: &AppDef, file: &Path) -> Result<PathBuf, String> {
    let tmp = util::data_dir().join("tmp").join(app.id);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| {
        run(
            "tar",
            &[
                "-xzf",
                &file.to_string_lossy(),
                "-C",
                &tmp.to_string_lossy(),
                "--no-same-owner",
            ],
        )?;
        // `cargo xtask dist` mete todo en una carpeta `<app>-<versión>-<plataforma>/`.
        let entries: Vec<PathBuf> = std::fs::read_dir(&tmp)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .collect();
        let root = match entries.as_slice() {
            [only] if only.is_dir() => only.clone(),
            _ => tmp.clone(),
        };
        let bin_name = app.linux_bins.first().copied().unwrap_or(app.id);
        if !root.join(bin_name).is_file() {
            return Err(format!("el archivo no contiene el programa {bin_name}"));
        }
        let dest = tarball_dir(app);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)
                .map_err(|e| format!("no se pudo reemplazar {}: {e}", dest.display()))?;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&root, &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        let bin = dest.join(bin_name);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        // Sin icono dentro del `.tar.gz`; la ventana se llama como el binario.
        let meta = DesktopMeta {
            icon: None,
            wm_class: Some(bin_name.to_string()),
        };
        write_desktop(app, &format!("\"{}\" %F", bin.display()), &meta);
        Ok(bin)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
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

/// Parámetros de Inno Setup para instalar o desinstalar sin preguntas (NSIS usa `/S`).
const INNO_SILENT: &str = "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART";

fn install_windows(app: &AppDef, file: &Path) -> Result<(), String> {
    let args = if app.packaging == Packaging::Inno {
        INNO_SILENT
    } else {
        "/S"
    };
    ps_run_wait(&file.to_string_lossy(), args)
}

/// `.msi`: `/passive` muestra el progreso y deja que Windows pida la elevación (con `/qn` un
/// instalador para todos los usuarios fallaría sin ella).
fn install_msi(file: &Path) -> Result<(), String> {
    let f = file.to_string_lossy().replace('\'', "''");
    ps_run_wait("msiexec.exe", &format!("/i \"{f}\" /passive /norestart"))
}

/// Instala la `.app` de un `.app.tar.gz` (Tauri) o de un `.dmg` (cargo-packager).
fn install_mac(app: &AppDef, file: &Path) -> Result<PathBuf, String> {
    let tmp = util::data_dir().join("tmp").join(app.id);
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
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
    let result = if file.extension().is_some_and(|x| x == "dmg") {
        install_mac_dmg(file, &tmp, &dest)
    } else {
        install_mac_tar(file, &tmp, &dest)
    };
    let _ = std::fs::remove_dir_all(&tmp);
    result?;
    let _ = run(
        "xattr",
        &["-dr", "com.apple.quarantine", &dest.to_string_lossy()],
    );
    Ok(dest)
}

fn find_app_bundle(dir: &Path) -> Result<PathBuf, String> {
    std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .ok_or_else(|| "el archivo no contiene una .app".into())
}

fn replace_app(dest: &Path) -> Result<(), String> {
    if dest.exists() {
        std::fs::remove_dir_all(dest)
            .map_err(|e| format!("no se pudo reemplazar {}: {e}", dest.display()))?;
    }
    Ok(())
}

fn install_mac_tar(file: &Path, tmp: &Path, dest: &Path) -> Result<(), String> {
    run(
        "tar",
        &[
            "-xzf",
            &file.to_string_lossy(),
            "-C",
            &tmp.to_string_lossy(),
        ],
    )?;
    let bundle = find_app_bundle(tmp)?;
    replace_app(dest)?;
    if std::fs::rename(&bundle, dest).is_err() {
        run("mv", &[&bundle.to_string_lossy(), &dest.to_string_lossy()])?;
    }
    Ok(())
}

/// Monta el `.dmg` sin abrir Finder, copia la `.app` con `ditto` y lo desmonta.
fn install_mac_dmg(file: &Path, tmp: &Path, dest: &Path) -> Result<(), String> {
    let mnt = tmp.join("dmg");
    std::fs::create_dir_all(&mnt).map_err(|e| e.to_string())?;
    let mnt_s = mnt.to_string_lossy();
    run(
        "hdiutil",
        &[
            "attach",
            "-nobrowse",
            "-readonly",
            "-noautoopen",
            "-mountpoint",
            &mnt_s,
            &file.to_string_lossy(),
        ],
    )?;
    let copied = find_app_bundle(&mnt).and_then(|bundle| {
        replace_app(dest)?;
        run(
            "ditto",
            &[&bundle.to_string_lossy(), &dest.to_string_lossy()],
        )
    });
    let _ = run("hdiutil", &["detach", &mnt_s, "-force"]);
    copied
}

pub fn uninstall(app: &AppDef, installed: &Installed) -> Result<(), String> {
    match (installed.format, &installed.launch) {
        (Format::AppImage, Launch::AppImage(p)) => {
            std::fs::remove_file(p).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(desktop_file(app));
            Ok(())
        }
        (Format::MacApp, Launch::MacApp(p)) => {
            // el arranque con la sesión que haya creado la app (IureVoz lo nombra con su
            // CFBundleIdentifier) apuntaría a una app que ya no existe
            if let (Some(id), Some(home)) = (mac_bundle_key(p, "CFBundleIdentifier"), util::home())
            {
                let _ = std::fs::remove_file(
                    home.join("Library/LaunchAgents")
                        .join(format!("{id}.plist")),
                );
            }
            std::fs::remove_dir_all(p).map_err(|e| e.to_string())
        }
        (Format::TarGz, Launch::Exec(p)) => {
            if !remove_tarball_dir(app, p) {
                return Err(format!("no se pudo quitar {}", tarball_dir(app).display()));
            }
            let _ = std::fs::remove_file(desktop_file(app));
            Ok(())
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
            // El desinstalador de Inno Setup es `unins000.exe`.
            let inno = Path::new(exe)
                .file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("unins"));
            ps_run_wait(exe, if inno { INNO_SILENT } else { "/S" })
        }
        (Format::Msi, _) => {
            let code = installed
                .uninstall
                .as_deref()
                .filter(|c| is_guid(c))
                .ok_or("no se encontró el código del producto")?;
            ps_run_wait("msiexec.exe", &format!("/x {code} /passive /norestart"))
        }
        _ => Err("no se sabe desinstalar este formato".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::Asset;

    #[test]
    fn product_codes_are_checked() {
        assert!(is_guid("{98436284-3DDF-71ED-BCCB-9A2D80415492}"));
        assert!(!is_guid("98436284-3DDF-71ED-BCCB-9A2D80415492"));
        assert!(!is_guid("{98436284-3DDF-71ED-BCCB-9A2D8041549Z}"));
        assert!(!is_guid("{x} & del C:\\"));
    }

    #[test]
    fn desktop_entry_names_icon_and_window_class() {
        let app = &crate::catalog::APPS[0];
        let meta = DesktopMeta {
            icon: Some(PathBuf::from("/d/icons/x.png")),
            wm_class: Some("iuredesk".into()),
        };
        let e = desktop_entry(app, "\"/a/x.AppImage\" %F", &meta);
        assert!(e.contains("Icon=/d/icons/x.png\n"), "{e}");
        assert!(e.contains("StartupWMClass=iuredesk\n"), "{e}");
        assert!(e.contains("X-Iureapps=true"));
        let plain = desktop_entry(app, "x", &DesktopMeta::default());
        assert!(plain.contains("Icon=application-x-executable"));
        assert!(!plain.contains("StartupWMClass"));
    }

    #[test]
    fn reads_keys_only_from_the_main_section() {
        let t = "[Desktop Entry]\nName=IureDesk\nStartupWMClass = iuredesk\n[Desktop Action compose]\nExec=y\n";
        assert_eq!(
            desktop_key(t, "StartupWMClass").as_deref(),
            Some("iuredesk")
        );
        assert_eq!(desktop_key(t, "Exec"), None);
        assert_eq!(desktop_key("StartupWMClass=x", "StartupWMClass"), None);
    }

    #[test]
    fn icon_kind_by_content() {
        assert_eq!(icon_ext(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(
            icon_ext(b"<?xml version='1.0'?><svg xmlns=''/>"),
            Some("svg")
        );
        assert_eq!(icon_ext(b"GIF89a"), None);
        assert_eq!(icon_ext(b""), None);
    }

    /// El `.tar.gz` de `cargo xtask dist` (carpeta `<app>-<ver>-x86_64-linux/` con los binarios).
    #[cfg(target_os = "linux")]
    #[test]
    fn installs_and_removes_tarball() {
        let base = std::env::temp_dir().join(format!("iureapps-tgz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::env::set_var("XDG_DATA_HOME", base.join("data"));
        let src = base.join("src/iureproject-0.2.0-x86_64-linux");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("iureproject"), "#!/bin/sh\n").unwrap();
        std::fs::write(src.join("iureproject-cli"), "#!/bin/sh\n").unwrap();
        let file = base.join("iureproject-0.2.0-x86_64-linux.tar.gz");
        run(
            "tar",
            &[
                "-czf",
                &file.to_string_lossy(),
                "-C",
                &base.join("src").to_string_lossy(),
                "iureproject-0.2.0-x86_64-linux",
            ],
        )
        .unwrap();

        let app = crate::catalog::find("iureproject").unwrap();
        let bin = install_tarball(app, &file).unwrap();
        assert_eq!(bin, util::data_dir().join("apps/iureproject/iureproject"));
        assert!(bin.with_file_name("iureproject-cli").exists());
        let desktop = std::fs::read_to_string(desktop_file(app)).unwrap();
        assert!(desktop.contains(&bin.display().to_string()));
        // Reinstalar reemplaza la carpeta.
        install_tarball(app, &file).unwrap();

        let installed = Installed {
            version: Some("0.2.0".into()),
            format: Format::TarGz,
            variant: None,
            launch: Launch::Exec(bin.clone()),
            uninstall: None,
        };
        uninstall(app, &installed).unwrap();
        assert!(!bin.exists() && !desktop_file(app).exists());
        let _ = std::fs::remove_dir_all(&base);
    }

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
