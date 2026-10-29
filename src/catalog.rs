//! Catálogo de apps de Iurefficient y la regla para escoger el instalador de cada equipo.
//!
//! Las apps Tauri publican sus instaladores como
//! `<Producto>_<versión>_<orden>-<plataforma>[-variante].<ext>`, p. ej.
//! `IureTranscribe_0.8.2_3-linux-x64-vulkan.AppImage`, junto con su `.sig` (minisign).

use crate::github::{Asset, Release};
use crate::system::{Arch, Os, PkgFamily, SystemInfo, Vendor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packaging {
    /// Instaladores de Tauri con el esquema de nombres de arriba.
    Tauri,
    /// Sólo `.deb` hecho con cargo-deb (`<crate>_<ver>-1_amd64.deb`).
    CargoDeb,
    /// Apps en Rust de la suite (`cargo xtask dist` / `package`), con `SHA256SUMS`:
    /// `<bin>-<ver>-x86_64-linux.tar.gz` y, con cargo-packager, `<bin>_<ver>_x86_64.AppImage`,
    /// `<bin>_<ver>_amd64.deb`, `<bin>_<ver>_x64-setup.exe` y `<Producto>_<ver>_<aarch64|x64>.dmg`.
    Packager,
}

pub struct AppDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Repositorio de GitHub de donde salen las releases (el de descargas si el código es privado).
    pub repo: &'static str,
    pub description: &'static str,
    /// Qué es, en pocas palabras (mosaico).
    pub tagline: &'static str,
    /// Segunda línea discreta del mosaico.
    pub hint: &'static str,
    /// Color de acento (RGB) para el mosaico.
    pub accent: [u8; 3],
    /// En exploración o desarrollo temprano (sección aparte del mosaico).
    pub experimental: bool,
    pub packaging: Packaging,
    pub oses: &'static [Os],
    /// Tiene variantes de aceleración (CPU / CUDA / Vulkan / Metal).
    pub gpu_variants: bool,
    /// Nombres posibles del ejecutable instalado por .deb/.rpm en /usr/bin.
    pub linux_bins: &'static [&'static str],
    /// Clave pública minisign del actualizador de Tauri (base64, como en tauri.conf.json).
    pub pubkey: Option<&'static str>,
    pub note: &'static str,
}

const KEY_8F1B: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDhGMUI0OEUxNjg1NjE1MjUKUldRbEZWWm80VWdiajlpRzB5WFRKY0RFZ01VVGN0TVMrV01LQjZWTUNTa3JsV0VoMFRWQTlmbmUK";
const KEY_E017: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEUwMTc4NjI1QkY2NjQyQ0IKUldUTFFtYS9KWVlYNEw1ZDQ0VEJNK2pQMGQ2UEJTSWJnZEhyOXZnVUYzWFRURUM5S1RJVmdpbGEK";
const ALL: &[Os] = &[Os::Linux, Os::Windows, Os::MacOs];

pub static APPS: &[AppDef] = &[
    AppDef {
        id: "iureditor",
        name: "iureditor",
        repo: "ellaguno/iureditor",
        description:
            "Markdown como texto enriquecido, con Mermaid y LaTeX; exporta a PDF, Word o HTML.",
        tagline: "Editor de Markdown",
        hint: "Exporta a PDF, Word y HTML",
        accent: [59, 130, 246],
        experimental: false,
        packaging: Packaging::Tauri,
        oses: ALL,
        gpu_variants: false,
        linux_bins: &["iureditor"],
        pubkey: Some(KEY_8F1B),
        note: "",
    },
    AppDef {
        id: "iuretranscribe",
        name: "IureTranscribe",
        repo: "ellaguno/iuretranscribe",
        description:
            "Transcribe reuniones y grabaciones en tu equipo con Whisper; resumen y minuta.",
        tagline: "Transcripción y minutas",
        hint: "Whisper local: CPU, CUDA, Vulkan o Metal",
        accent: [20, 184, 166],
        experimental: false,
        packaging: Packaging::Tauri,
        oses: ALL,
        gpu_variants: true,
        linux_bins: &["iuretranscribe", "IureTranscribe", "iure-transcribe"],
        pubkey: Some(KEY_8F1B),
        note: "",
    },
    AppDef {
        id: "iureocr",
        name: "IureOCR",
        repo: "ellaguno/iureocr",
        description:
            "Convierte escaneos y fotos de documentos en PDF con texto buscable (Tesseract).",
        tagline: "OCR de documentos",
        hint: "PDF con texto buscable, con Tesseract",
        accent: [239, 68, 68],
        experimental: false,
        packaging: Packaging::Tauri,
        oses: ALL,
        gpu_variants: false,
        linux_bins: &["iureocr", "IureOCR", "iure-ocr"],
        pubkey: Some(KEY_E017),
        note: "",
    },
    AppDef {
        id: "iuredav",
        name: "IureDav",
        repo: "ellaguno/iuredav",
        description: "Tu servidor WebDAV (o Iurefficient) como una carpeta más del equipo.",
        tagline: "Unidad WebDAV",
        hint: "Iurefficient como una carpeta del equipo",
        accent: [245, 158, 11],
        experimental: false,
        packaging: Packaging::Tauri,
        oses: ALL,
        gpu_variants: false,
        linux_bins: &["iuredav-app", "IureDav", "iuredav", "iure-dav"],
        pubkey: Some(KEY_8F1B),
        note: "En Windows instala para todo el equipo: pedirá permiso de administrador.",
    },
    AppDef {
        id: "iurevoz",
        name: "IureVoz",
        repo: "ellaguno/iurevoz",
        description: "Dictado y control del escritorio por voz, local y sólo CPU.",
        tagline: "Dictado por voz",
        hint: "Local y sin GPU, para GNOME en Wayland",
        accent: [139, 92, 246],
        experimental: true,
        packaging: Packaging::CargoDeb,
        oses: &[Os::Linux],
        gpu_variants: false,
        linux_bins: &["iurevoz"],
        pubkey: None,
        note: "Sólo Linux (.deb), pensado para GNOME en Wayland.",
    },
    AppDef {
        id: "iuredesk",
        name: "IureDesk",
        repo: "ellaguno/iuredesk-releases",
        description: "Correo, calendario, contactos, notas y tareas, integrado con Iurefficient.",
        tagline: "Correo y agenda",
        hint: "Correo, calendario, contactos y tareas",
        accent: [14, 165, 233],
        experimental: true,
        packaging: Packaging::Packager,
        oses: ALL,
        gpu_variants: false,
        linux_bins: &["iuredesk"],
        pubkey: None,
        note: "Versión de prueba.",
    },
    AppDef {
        id: "iureproject",
        name: "IureProject",
        repo: "ellaguno/iureproject-releases",
        description: "Gestión de proyectos en Rust puro, compatible con Microsoft Project.",
        tagline: "Gestión de proyectos",
        hint: "Compatible con archivos de MS Project",
        accent: [34, 197, 94],
        experimental: true,
        packaging: Packaging::Packager,
        oses: &[Os::Linux],
        gpu_variants: false,
        linux_bins: &["iureproject"],
        pubkey: None,
        note: "En desarrollo. Sólo Linux x64 (binario .tar.gz).",
    },
];

pub fn find(id: &str) -> Option<&'static AppDef> {
    APPS.iter().find(|a| a.id.eq_ignore_ascii_case(id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Variant {
    Cpu,
    Cuda,
    Vulkan,
    /// Apple Silicon (whisper.cpp con Metal).
    Metal,
}

impl Variant {
    pub fn label(self) -> &'static str {
        match self {
            Variant::Cpu => "CPU",
            Variant::Cuda => "NVIDIA CUDA",
            Variant::Vulkan => "GPU Vulkan",
            Variant::Metal => "Apple Metal",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cpu" => Some(Variant::Cpu),
            "cuda" | "nvidia" => Some(Variant::Cuda),
            "vulkan" => Some(Variant::Vulkan),
            "metal" => Some(Variant::Metal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Format {
    AppImage,
    Deb,
    Rpm,
    /// Instalador NSIS de Windows (`.exe`, admite `/S`).
    WinExe,
    /// `.app.tar.gz` de macOS.
    MacApp,
    /// Binarios sueltos en `.tar.gz` (Linux), sin instalador.
    TarGz,
}

impl Format {
    pub fn label(self) -> &'static str {
        match self {
            Format::AppImage => "AppImage",
            Format::Deb => ".deb",
            Format::Rpm => ".rpm",
            Format::WinExe => "Instalador .exe",
            Format::MacApp => ".app",
            Format::TarGz => "Binario .tar.gz",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Format::AppImage => ".AppImage",
            Format::Deb => ".deb",
            Format::Rpm => ".rpm",
            Format::WinExe => ".exe",
            Format::MacApp => ".app.tar.gz",
            Format::TarGz => ".tar.gz",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().trim_start_matches('.') {
            "appimage" => Some(Format::AppImage),
            "deb" => Some(Format::Deb),
            "rpm" => Some(Format::Rpm),
            "exe" => Some(Format::WinExe),
            "app" => Some(Format::MacApp),
            "tar.gz" | "targz" | "tar" => Some(Format::TarGz),
            _ => None,
        }
    }
}

impl AppDef {
    pub fn supports(&self, sys: &SystemInfo) -> bool {
        self.oses.contains(&sys.os)
    }

    /// Variantes que tienen sentido en este sistema operativo.
    pub fn variants(&self, sys: &SystemInfo) -> Vec<Variant> {
        if !self.gpu_variants {
            return vec![Variant::Cpu];
        }
        match (sys.os, sys.arch) {
            (Os::MacOs, Arch::Arm64) => vec![Variant::Metal, Variant::Cpu],
            (Os::MacOs, _) => vec![Variant::Cpu],
            _ => vec![Variant::Cpu, Variant::Vulkan, Variant::Cuda],
        }
    }

    /// Formatos posibles por orden de preferencia en este sistema.
    pub fn formats(&self, sys: &SystemInfo) -> Vec<Format> {
        match sys.os {
            Os::Windows => vec![Format::WinExe],
            Os::MacOs => vec![Format::MacApp],
            Os::Linux => {
                let mut f = vec![Format::AppImage, Format::Deb, Format::Rpm];
                // Sin FUSE el AppImage funciona (se extrae al abrir) pero arranca más lento.
                match sys.pkg {
                    PkgFamily::Deb if !sys.fuse => f = vec![Format::Deb, Format::AppImage],
                    PkgFamily::Rpm if !sys.fuse => f = vec![Format::Rpm, Format::AppImage],
                    _ => {}
                }
                if sys.pkg != PkgFamily::Deb {
                    f.retain(|x| *x != Format::Deb);
                }
                if sys.pkg != PkgFamily::Rpm {
                    f.retain(|x| *x != Format::Rpm);
                }
                // Último recurso: sólo lo publican las apps en Rust de la suite.
                f.push(Format::TarGz);
                f
            }
            Os::Other => vec![],
        }
    }

    /// Variante recomendada para este equipo y el motivo.
    pub fn recommend_variant(&self, sys: &SystemInfo) -> (Variant, String) {
        if !self.gpu_variants {
            return (Variant::Cpu, String::new());
        }
        if sys.os == Os::MacOs {
            return if sys.arch == Arch::Arm64 {
                (
                    Variant::Metal,
                    "Apple Silicon: whisper.cpp usa la GPU con Metal.".into(),
                )
            } else {
                (Variant::Cpu, "Mac Intel: sólo hay versión CPU.".into())
            };
        }
        if let Some(n) = &sys.nvidia {
            let driver_ok = sys.nvidia_driver_major().map(|m| m >= 525).unwrap_or(false);
            if !driver_ok {
                return (
                    Variant::Vulkan,
                    format!(
                        "{} con driver {}: CUDA 12 necesita driver ≥ 525; se usa Vulkan.",
                        n.name, n.driver
                    ),
                );
            }
            if sys.os == Os::Linux && sys.pkg != PkgFamily::Deb {
                return (
                    Variant::Vulkan,
                    format!("{}: la versión CUDA de Linux sólo existe en .deb; en esta distribución se usa Vulkan.", n.name),
                );
            }
            return (
                Variant::Cuda,
                format!("{} (driver {}): la más rápida es CUDA.", n.name, n.driver),
            );
        }
        if sys.has_gpu(Vendor::Nvidia) {
            return if sys.vulkan {
                (
                    Variant::Vulkan,
                    "GPU NVIDIA sin driver propietario (nvidia-smi no responde): Vulkan.".into(),
                )
            } else {
                (
                    Variant::Cpu,
                    "GPU NVIDIA sin driver: instala el driver de NVIDIA para usar CUDA.".into(),
                )
            };
        }
        if sys.vulkan && (sys.has_gpu(Vendor::Amd) || sys.has_gpu(Vendor::Intel)) {
            let gpu = sys
                .gpus
                .iter()
                .find(|g| matches!(g.vendor, Vendor::Amd | Vendor::Intel))
                .map(|g| g.name.as_str())
                .unwrap_or("GPU");
            return (
                Variant::Vulkan,
                format!("{gpu} con Vulkan: acelera Whisper sin CUDA."),
            );
        }
        let why = if sys.gpus.is_empty() {
            "No se detectó GPU"
        } else {
            "Sin Vulkan ni CUDA disponibles"
        };
        (Variant::Cpu, format!("{why}: versión CPU."))
    }

    /// Instalador (y su firma, si existe) para el equipo, la variante y el formato pedidos.
    pub fn select_asset<'a>(
        &self,
        release: &'a Release,
        sys: &SystemInfo,
        variant: Variant,
        format: Format,
    ) -> Option<(&'a Asset, Option<&'a Asset>)> {
        let asset = match self.packaging {
            Packaging::CargoDeb => {
                let arch = match sys.arch {
                    Arch::X64 => "_amd64.deb",
                    Arch::Arm64 => "_arm64.deb",
                    Arch::Other => return None,
                };
                if sys.os != Os::Linux || format != Format::Deb {
                    return None;
                }
                release.assets.iter().find(|a| a.name.ends_with(arch))?
            }
            Packaging::Packager => {
                let ends: &[&str] = match (sys.os, sys.arch, format) {
                    (Os::Linux, Arch::X64, Format::AppImage) => &["_x86_64.AppImage"],
                    (Os::Linux, Arch::X64, Format::Deb) => &["_amd64.deb"],
                    (Os::Linux, Arch::Arm64, Format::AppImage) => &["_aarch64.AppImage"],
                    (Os::Linux, Arch::Arm64, Format::Deb) => &["_arm64.deb"],
                    (Os::Linux, Arch::X64, Format::TarGz) => &["-x86_64-linux.tar.gz"],
                    (Os::Linux, Arch::Arm64, Format::TarGz) => &["-aarch64-linux.tar.gz"],
                    (Os::Windows, Arch::Arm64, Format::WinExe) => {
                        &["_arm64-setup.exe", "_x64-setup.exe"]
                    }
                    (Os::Windows, _, Format::WinExe) => &["_x64-setup.exe"],
                    (Os::MacOs, Arch::Arm64, Format::MacApp) => &["_aarch64.dmg", "_universal.dmg"],
                    (Os::MacOs, Arch::X64, Format::MacApp) => &["_x64.dmg", "_universal.dmg"],
                    _ => return None,
                };
                ends.iter()
                    .find_map(|e| release.assets.iter().find(|a| a.name.ends_with(e)))?
            }
            Packaging::Tauri => {
                let suffix = match variant {
                    Variant::Cuda => "-cuda",
                    Variant::Vulkan => "-vulkan",
                    Variant::Cpu | Variant::Metal => "",
                };
                let platforms: &[&str] = match (sys.os, sys.arch) {
                    (Os::Linux, Arch::X64) => &["linux-x64"],
                    (Os::Windows, Arch::X64) => &["windows-x64"],
                    // Windows ARM ejecuta binarios x64 emulados.
                    (Os::Windows, Arch::Arm64) => &["windows-x64"],
                    (Os::MacOs, Arch::Arm64) if variant == Variant::Metal => &["macos-arm64"],
                    (Os::MacOs, Arch::Arm64) => &["macos-universal", "macos-arm64"],
                    (Os::MacOs, Arch::X64) => &["macos-x64", "macos-universal"],
                    _ => return None,
                };
                platforms.iter().find_map(|p| {
                    let end = format!("-{p}{suffix}{}", format.ext());
                    release.assets.iter().find(|a| a.name.ends_with(&end))
                })?
            }
        };
        let sig_name = format!("{}.sig", asset.name);
        let sig = release.assets.iter().find(|a| a.name == sig_name);
        Some((asset, sig))
    }

    /// Primer formato disponible en la release para esta variante.
    pub fn best_format(
        &self,
        release: &Release,
        sys: &SystemInfo,
        variant: Variant,
    ) -> Option<Format> {
        self.formats(sys)
            .into_iter()
            .find(|f| self.select_asset(release, sys, variant, *f).is_some())
    }

    pub fn release_url(&self) -> String {
        format!("https://github.com/{}/releases", self.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{Gpu, Nvidia};

    fn release(app: &str, ver: &str, plats: &[&str]) -> Release {
        let assets = plats
            .iter()
            .flat_map(|p| {
                let name = format!("{app}_{ver}_{p}");
                [name.clone(), format!("{name}.sig")]
            })
            .map(|name| Asset {
                size: 1,
                browser_download_url: String::new(),
                url: String::new(),
                name,
            })
            .collect();
        Release {
            tag_name: format!("v{ver}"),
            html_url: String::new(),
            body: None,
            published_at: None,
            assets,
        }
    }

    fn transcribe_release() -> Release {
        release(
            "IureTranscribe",
            "0.8.2",
            &[
                "1-windows-x64.exe",
                "1-windows-x64.msi",
                "1-windows-x64-cuda.exe",
                "1-windows-x64-vulkan.exe",
                "2-macos-arm64.app.tar.gz",
                "2-macos-arm64.dmg",
                "2-macos-x64.app.tar.gz",
                "3-linux-x64.AppImage",
                "3-linux-x64.deb",
                "3-linux-x64.rpm",
                "3-linux-x64-vulkan.AppImage",
                "3-linux-x64-vulkan.deb",
                "3-linux-x64-cuda.deb",
            ],
        )
    }

    fn sys(os: Os, arch: Arch) -> SystemInfo {
        SystemInfo {
            os,
            arch,
            os_name: String::new(),
            pkg: PkgFamily::Deb,
            desktop: String::new(),
            session: String::new(),
            cpu: String::new(),
            cores: 8,
            avx2: Some(true),
            ram_gb: Some(16.0),
            gpus: vec![],
            nvidia: None,
            vulkan: false,
            fuse: true,
        }
    }

    fn pick(app: &str, r: &Release, s: &SystemInfo, v: Variant, f: Format) -> Option<String> {
        find(app)
            .unwrap()
            .select_asset(r, s, v, f)
            .map(|(a, _)| a.name.clone())
    }

    #[test]
    fn picks_transcribe_variants() {
        let r = transcribe_release();
        let linux = sys(Os::Linux, Arch::X64);
        assert_eq!(
            pick("iuretranscribe", &r, &linux, Variant::Cpu, Format::AppImage).unwrap(),
            "IureTranscribe_0.8.2_3-linux-x64.AppImage"
        );
        assert_eq!(
            pick(
                "iuretranscribe",
                &r,
                &linux,
                Variant::Vulkan,
                Format::AppImage
            )
            .unwrap(),
            "IureTranscribe_0.8.2_3-linux-x64-vulkan.AppImage"
        );
        assert!(pick(
            "iuretranscribe",
            &r,
            &linux,
            Variant::Cuda,
            Format::AppImage
        )
        .is_none());
        let app = find("iuretranscribe").unwrap();
        assert_eq!(
            app.best_format(&r, &linux, Variant::Cuda),
            Some(Format::Deb)
        );
        let win = sys(Os::Windows, Arch::X64);
        assert_eq!(
            pick("iuretranscribe", &r, &win, Variant::Cuda, Format::WinExe).unwrap(),
            "IureTranscribe_0.8.2_1-windows-x64-cuda.exe"
        );
        let mac = sys(Os::MacOs, Arch::Arm64);
        assert_eq!(
            pick("iuretranscribe", &r, &mac, Variant::Metal, Format::MacApp).unwrap(),
            "IureTranscribe_0.8.2_2-macos-arm64.app.tar.gz"
        );
        // La firma acompaña al instalador.
        let (_, sig) = app
            .select_asset(&r, &win, Variant::Vulkan, Format::WinExe)
            .unwrap();
        assert_eq!(
            sig.unwrap().name,
            "IureTranscribe_0.8.2_1-windows-x64-vulkan.exe.sig"
        );
    }

    #[test]
    fn picks_universal_mac_and_cargo_deb() {
        let r = release(
            "iureditor",
            "1.13.0",
            &["2-macos-universal.app.tar.gz", "3-linux-x64.AppImage"],
        );
        assert_eq!(
            pick(
                "iureditor",
                &r,
                &sys(Os::MacOs, Arch::X64),
                Variant::Cpu,
                Format::MacApp
            )
            .unwrap(),
            "iureditor_1.13.0_2-macos-universal.app.tar.gz"
        );
        assert_eq!(
            pick(
                "iureditor",
                &r,
                &sys(Os::MacOs, Arch::Arm64),
                Variant::Cpu,
                Format::MacApp
            )
            .unwrap(),
            "iureditor_1.13.0_2-macos-universal.app.tar.gz"
        );
        let voz = Release {
            tag_name: "v0.2.1".into(),
            html_url: String::new(),
            body: None,
            published_at: None,
            assets: vec![Asset {
                name: "iurevoz_0.2.1-1_amd64.deb".into(),
                size: 1,
                browser_download_url: String::new(),
                url: String::new(),
            }],
        };
        assert!(pick(
            "iurevoz",
            &voz,
            &sys(Os::Linux, Arch::X64),
            Variant::Cpu,
            Format::Deb
        )
        .is_some());
        assert!(pick(
            "iurevoz",
            &voz,
            &sys(Os::Windows, Arch::X64),
            Variant::Cpu,
            Format::Deb
        )
        .is_none());
    }

    #[test]
    fn picks_packager_assets() {
        let names = [
            "iuredesk-0.3.0-x86_64-linux.tar.gz",
            "iuredesk_0.3.0_amd64.deb",
            "iuredesk_0.3.0_x86_64.AppImage",
            "iuredesk_0.3.0_x64-setup.exe",
            "IureDesk_0.3.0_aarch64.dmg",
            "IureDesk_0.3.0_x64.dmg",
            "SHA256SUMS",
        ];
        let r = Release {
            tag_name: "v0.3.0".into(),
            html_url: String::new(),
            body: None,
            published_at: None,
            assets: names
                .iter()
                .map(|n| Asset {
                    name: n.to_string(),
                    size: 1,
                    browser_download_url: String::new(),
                    url: String::new(),
                })
                .collect(),
        };
        let linux = sys(Os::Linux, Arch::X64);
        let p = |s: &SystemInfo, f| pick("iuredesk", &r, s, Variant::Cpu, f);
        assert_eq!(
            p(&linux, Format::AppImage).unwrap(),
            "iuredesk_0.3.0_x86_64.AppImage"
        );
        assert_eq!(p(&linux, Format::Deb).unwrap(), "iuredesk_0.3.0_amd64.deb");
        assert!(p(&linux, Format::Rpm).is_none());
        let win = sys(Os::Windows, Arch::Arm64);
        assert_eq!(
            p(&win, Format::WinExe).unwrap(),
            "iuredesk_0.3.0_x64-setup.exe"
        );
        let mac = sys(Os::MacOs, Arch::Arm64);
        assert_eq!(
            p(&mac, Format::MacApp).unwrap(),
            "IureDesk_0.3.0_aarch64.dmg"
        );
        let mac = sys(Os::MacOs, Arch::X64);
        assert_eq!(p(&mac, Format::MacApp).unwrap(), "IureDesk_0.3.0_x64.dmg");
        assert_eq!(
            p(&linux, Format::TarGz).unwrap(),
            "iuredesk-0.3.0-x86_64-linux.tar.gz"
        );
        // IureProject sólo publica el .tar.gz de `cargo xtask dist`.
        let mut r = r.clone();
        r.assets.retain(|a| a.name.ends_with(".tar.gz"));
        for a in &mut r.assets {
            a.name = "iureproject-0.2.0-x86_64-linux.tar.gz".into();
        }
        let app = find("iureproject").unwrap();
        assert_eq!(
            app.best_format(&r, &linux, Variant::Cpu),
            Some(Format::TarGz)
        );
        assert!(app
            .best_format(&r, &sys(Os::Windows, Arch::X64), Variant::Cpu)
            .is_none());
    }

    #[test]
    fn recommends_by_hardware() {
        let app = find("iuretranscribe").unwrap();
        let mut s = sys(Os::Linux, Arch::X64);
        assert_eq!(app.recommend_variant(&s).0, Variant::Cpu);
        s.vulkan = true;
        s.gpus = vec![Gpu {
            vendor: Vendor::Amd,
            name: "Radeon RX 7600".into(),
        }];
        assert_eq!(app.recommend_variant(&s).0, Variant::Vulkan);
        s.gpus.push(Gpu {
            vendor: Vendor::Nvidia,
            name: "RTX 4070".into(),
        });
        s.nvidia = Some(Nvidia {
            name: "RTX 4070".into(),
            driver: "570.86".into(),
            vram_mb: Some(12282),
        });
        assert_eq!(app.recommend_variant(&s).0, Variant::Cuda);
        s.pkg = PkgFamily::Rpm;
        assert_eq!(app.recommend_variant(&s).0, Variant::Vulkan);
        s.pkg = PkgFamily::Deb;
        s.nvidia.as_mut().unwrap().driver = "470.256".into();
        assert_eq!(app.recommend_variant(&s).0, Variant::Vulkan);
        assert_eq!(
            app.recommend_variant(&sys(Os::MacOs, Arch::Arm64)).0,
            Variant::Metal
        );
    }
}
