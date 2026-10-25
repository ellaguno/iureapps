//! Detección del equipo: sistema, CPU, memoria y GPU (para escoger la variante adecuada).

use crate::util::{output, which};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum Os {
    Linux,
    Windows,
    MacOs,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    X64,
    Arm64,
    Other,
}

/// Familia de paquetes del sistema Linux.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgFamily {
    Deb,
    Rpm,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Other,
}

#[derive(Clone, Debug)]
pub struct Gpu {
    pub vendor: Vendor,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Nvidia {
    pub name: String,
    pub driver: String,
    pub vram_mb: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct SystemInfo {
    pub os: Os,
    pub arch: Arch,
    pub os_name: String,
    pub pkg: PkgFamily,
    pub desktop: String,
    pub session: String,
    pub cpu: String,
    pub cores: usize,
    /// `None` fuera de x86.
    pub avx2: Option<bool>,
    pub ram_gb: Option<f64>,
    pub gpus: Vec<Gpu>,
    /// Presente si `nvidia-smi` responde, es decir, hay driver de NVIDIA funcionando.
    pub nvidia: Option<Nvidia>,
    pub vulkan: bool,
    /// Linux: hay FUSE para montar AppImages (si no, se ejecutan extrayéndose).
    pub fuse: bool,
}

impl SystemInfo {
    pub fn detect() -> Self {
        let os = match std::env::consts::OS {
            "linux" => Os::Linux,
            "windows" => Os::Windows,
            "macos" => Os::MacOs,
            _ => Os::Other,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X64,
            "aarch64" => Arch::Arm64,
            _ => Arch::Other,
        };
        let (os_name, pkg) = os_name_and_pkg(os);
        let mut gpus = detect_gpus(os);
        let nvidia = detect_nvidia();
        if let Some(n) = &nvidia {
            if !gpus.iter().any(|g| g.vendor == Vendor::Nvidia) {
                gpus.push(Gpu {
                    vendor: Vendor::Nvidia,
                    name: n.name.clone(),
                });
            }
        }
        SystemInfo {
            os,
            arch,
            os_name,
            pkg,
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            session: std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
            cpu: cpu_name(os),
            cores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            avx2: avx2(),
            ram_gb: ram_bytes(os).map(|b| b as f64 / 1_073_741_824.0),
            gpus,
            nvidia,
            vulkan: has_vulkan(os),
            fuse: os != Os::Linux || has_fuse(),
        }
    }

    pub fn has_gpu(&self, vendor: Vendor) -> bool {
        self.gpus.iter().any(|g| g.vendor == vendor)
    }

    /// Major del driver de NVIDIA (p. ej. 570).
    pub fn nvidia_driver_major(&self) -> Option<u32> {
        self.nvidia.as_ref()?.driver.split('.').next()?.parse().ok()
    }

    pub fn platform_label(&self) -> String {
        let arch = match self.arch {
            Arch::X64 => "x86_64",
            Arch::Arm64 => "arm64",
            Arch::Other => std::env::consts::ARCH,
        };
        format!("{} ({arch})", self.os_name)
    }
}

fn os_name_and_pkg(os: Os) -> (String, PkgFamily) {
    match os {
        Os::Linux => {
            let text = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
            let field = |key: &str| {
                text.lines()
                    .find_map(|l| l.strip_prefix(&format!("{key}=")))
                    .map(|v| v.trim_matches('"').to_string())
                    .unwrap_or_default()
            };
            let ids = format!("{} {}", field("ID"), field("ID_LIKE")).to_lowercase();
            let pkg = if ["debian", "ubuntu"].iter().any(|d| ids.contains(d)) {
                PkgFamily::Deb
            } else if ["fedora", "rhel", "suse", "centos"]
                .iter()
                .any(|d| ids.contains(d))
            {
                PkgFamily::Rpm
            } else if which("dpkg").is_some() {
                PkgFamily::Deb
            } else if which("rpm").is_some() {
                PkgFamily::Rpm
            } else {
                PkgFamily::Other
            };
            let name = field("PRETTY_NAME");
            (
                if name.is_empty() {
                    "Linux".into()
                } else {
                    name
                },
                pkg,
            )
        }
        Os::MacOs => {
            let v = output("sw_vers", &["-productVersion"]).unwrap_or_default();
            (format!("macOS {v}").trim().to_string(), PkgFamily::Other)
        }
        Os::Windows => {
            let caption = output(
                "powershell",
                &[
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_OperatingSystem).Caption",
                ],
            );
            (
                caption.unwrap_or_else(|| "Windows".into()),
                PkgFamily::Other,
            )
        }
        Os::Other => (std::env::consts::OS.into(), PkgFamily::Other),
    }
}

fn cpu_name(os: Os) -> String {
    let name = match os {
        Os::Linux => std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
            t.lines()
                .find(|l| l.starts_with("model name") || l.starts_with("Model"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        }),
        Os::MacOs => output("sysctl", &["-n", "machdep.cpu.brand_string"]),
        Os::Windows => std::env::var("PROCESSOR_IDENTIFIER").ok(),
        Os::Other => None,
    };
    name.unwrap_or_else(|| "desconocido".into())
}

fn avx2() -> Option<bool> {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    {
        Some(std::arch::is_x86_feature_detected!("avx2"))
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    {
        None
    }
}

fn ram_bytes(os: Os) -> Option<u64> {
    match os {
        Os::Linux => {
            let t = std::fs::read_to_string("/proc/meminfo").ok()?;
            let kb: u64 = t
                .lines()
                .find(|l| l.starts_with("MemTotal:"))?
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()?;
            Some(kb * 1024)
        }
        Os::MacOs => output("sysctl", &["-n", "hw.memsize"])?.parse().ok(),
        Os::Windows => output(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ],
        )?
        .parse()
        .ok(),
        Os::Other => None,
    }
}

fn vendor_of(text: &str) -> Vendor {
    let t = text.to_lowercase();
    if t.contains("nvidia") || t.contains("0x10de") {
        Vendor::Nvidia
    } else if t.contains("amd")
        || t.contains("radeon")
        || t.contains("ati ")
        || t.contains("0x1002")
    {
        Vendor::Amd
    } else if t.contains("intel") || t.contains("0x8086") {
        Vendor::Intel
    } else if t.contains("apple") {
        Vendor::Apple
    } else {
        Vendor::Other
    }
}

fn detect_gpus(os: Os) -> Vec<Gpu> {
    let mut gpus = Vec::new();
    match os {
        Os::Linux => {
            // Con lspci se obtiene el nombre; si no está, basta el fabricante de /sys.
            if let Some(out) = output("lspci", &[]) {
                for line in out.lines() {
                    let l = line.to_lowercase();
                    if l.contains("vga compatible")
                        || l.contains("3d controller")
                        || l.contains("display controller")
                    {
                        let name = line
                            .splitn(3, ':')
                            .nth(2)
                            .unwrap_or(line)
                            .trim()
                            .to_string();
                        gpus.push(Gpu {
                            vendor: vendor_of(&name),
                            name,
                        });
                    }
                }
            }
            if gpus.is_empty() {
                if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if !name.starts_with("card") || name.contains('-') {
                            continue;
                        }
                        if let Ok(v) = std::fs::read_to_string(e.path().join("device/vendor")) {
                            let vendor = vendor_of(v.trim());
                            gpus.push(Gpu {
                                vendor,
                                name: format!("{vendor:?} ({})", v.trim()),
                            });
                        }
                    }
                }
            }
        }
        Os::Windows => {
            if let Some(out) = output(
                "powershell",
                &[
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_VideoController).Name",
                ],
            ) {
                for name in out.lines().map(str::trim).filter(|l| !l.is_empty()) {
                    // Adaptadores virtuales (Escritorio remoto, Parsec…) no cuentan.
                    if name.contains("Basic Display") || name.contains("Remote") {
                        continue;
                    }
                    gpus.push(Gpu {
                        vendor: vendor_of(name),
                        name: name.to_string(),
                    });
                }
            }
        }
        Os::MacOs => {
            if let Some(out) = output("system_profiler", &["SPDisplaysDataType"]) {
                for l in out.lines() {
                    if let Some(name) = l.trim().strip_prefix("Chipset Model:") {
                        let name = name.trim().to_string();
                        gpus.push(Gpu {
                            vendor: vendor_of(&name),
                            name,
                        });
                    }
                }
            }
        }
        Os::Other => {}
    }
    gpus
}

fn detect_nvidia() -> Option<Nvidia> {
    let out = output(
        "nvidia-smi",
        &[
            "--query-gpu=name,driver_version,memory.total",
            "--format=csv,noheader,nounits",
        ],
    )?;
    let line = out.lines().next()?;
    let mut parts = line.split(',').map(str::trim);
    Some(Nvidia {
        name: parts.next()?.to_string(),
        driver: parts.next().unwrap_or_default().to_string(),
        vram_mb: parts.next().and_then(|v| v.parse().ok()),
    })
}

fn has_vulkan(os: Os) -> bool {
    match os {
        Os::Linux => {
            let loader = [
                "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
                "/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
                "/usr/lib64/libvulkan.so.1",
                "/usr/lib/libvulkan.so.1",
            ]
            .iter()
            .any(|p| Path::new(p).exists());
            // Además del cargador hace falta al menos un driver (ICD) instalado.
            let icd = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"]
                .iter()
                .any(|d| {
                    std::fs::read_dir(d)
                        .map(|mut r| r.next().is_some())
                        .unwrap_or(false)
                });
            loader && icd
        }
        Os::Windows => {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
            Path::new(&root)
                .join("System32")
                .join("vulkan-1.dll")
                .exists()
        }
        _ => false,
    }
}

fn has_fuse() -> bool {
    let lib = [
        "/usr/lib/x86_64-linux-gnu/libfuse.so.2",
        "/usr/lib64/libfuse.so.2",
        "/usr/lib/libfuse.so.2",
        "/lib/x86_64-linux-gnu/libfuse.so.2",
    ]
    .iter()
    .any(|p| Path::new(p).exists());
    lib && Path::new("/dev/fuse").exists()
}
