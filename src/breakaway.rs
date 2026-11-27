//! Procesos fuera del contenedor MSIX.
//!
//! Instalado desde Microsoft Store, el lanzador corre con identidad de paquete y, como cualquier
//! app empaquetada, lo que crea en `AppData` y lo que escribe en `HKCU` queda virtualizado (en
//! una copia privada del paquete que nadie más ve). Sus procesos hijos heredan ese contenedor.
//! Para que lo que instala, desinstala o abre afecte al sistema real, esos procesos se crean
//! con `PROC_THREAD_ATTRIBUTE_DESKTOP_APP_POLICY` =
//! `PROCESS_CREATION_DESKTOP_APP_BREAKAWAY_ENABLE_PROCESS_TREE` (Windows 10 1809 o posterior):
//! el proceso y todo su árbol (también lo que se eleve con UAC) corren fuera del paquete.
//!
//! Fuera de la Store todo se lanza como siempre, con [`crate::util::command`].

use std::path::{Path, PathBuf};

/// Si los procesos que deben tocar el sistema real hay que sacarlos del contenedor: sólo
/// cuando el lanzador corre empaquetado (MSIX).
pub fn needed() -> bool {
    crate::selfupdate::from_store()
}

/// Ejecuta un programa esperando a que termine, como [`crate::util::run`] (sin ventana; el
/// error lleva stderr), pero fuera del contenedor si el lanzador está empaquetado.
pub fn run(program: &str, args: &[&str]) -> Result<(), String> {
    if !needed() {
        return crate::util::run(program, args);
    }
    #[cfg(windows)]
    {
        let (code, stderr) =
            imp::run(program, args).map_err(|e| format!("no se pudo ejecutar {program}: {e}"))?;
        if code == 0 {
            Ok(())
        } else {
            Err(format!(
                "{program} terminó con código de salida: {code}: {}",
                stderr.trim()
            ))
        }
    }
    #[cfg(not(windows))]
    unreachable!("sólo hay paquete MSIX en Windows")
}

/// Abre un programa sin esperar, fuera del contenedor (sólo se usa empaquetado, en Windows).
pub fn spawn(program: &Path, args: &[&str]) -> Result<(), String> {
    #[cfg(windows)]
    {
        imp::spawn(&program.to_string_lossy(), args).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (program, args);
        Err("sólo hay paquete MSIX en Windows".into())
    }
}

// Un ejecutable de la carpeta del paquete (este mismo `iureapps.exe`) vuelve a correr dentro
// del contenedor aunque se lance así; para tocar archivos reales se usa PowerShell.

fn ps_quote(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "''"))
}

/// Copia `src` a `dest` (creando su carpeta) desde fuera del contenedor, para que `dest` quede
/// en el `AppData` real y no en la copia privada del paquete.
pub fn copy_file(src: &Path, dest: &Path) -> Result<(), String> {
    let dir = dest.parent().ok_or("ruta sin carpeta")?;
    let script = format!(
        "$ErrorActionPreference = 'Stop'; New-Item -ItemType Directory -Force -Path {} | Out-Null; \
         Copy-Item -LiteralPath {} -Destination {} -Force",
        ps_quote(dir),
        ps_quote(src),
        ps_quote(dest)
    );
    run(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )
}

/// Borra `path` (si existe) desde fuera del contenedor.
pub fn remove_file(path: &Path) -> Result<(), String> {
    let p = ps_quote(path);
    let script = format!(
        "$ErrorActionPreference = 'Stop'; if (Test-Path -LiteralPath {p}) {{ Remove-Item -LiteralPath {p} -Force }}"
    );
    run(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )
}

/// Carpeta de datos del lanzador cuando corre empaquetado:
/// `%LOCALAPPDATA%\Packages\<familia>\LocalCache\Local\iureapps`, justo donde Windows deja lo
/// que el paquete crea en `%LOCALAPPDATA%\iureapps`. Con la ruta real, los instaladores que se
/// lanzan fuera del contenedor encuentran lo descargado.
pub fn package_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let family = imp::package_family_name()?;
            let local = std::env::var_os("LOCALAPPDATA")?;
            Some(
                PathBuf::from(local)
                    .join("Packages")
                    .join(family)
                    .join(r"LocalCache\Local\iureapps"),
            )
        })
        .clone()
    }
    #[cfg(not(windows))]
    None
}

/// Línea de órdenes de Windows con las reglas de `CommandLineToArgvW` (las mismas que usa
/// `std::process::Command`): comillas si hace falta, `\"` y barras antes de comillas dobladas.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn command_line(program: &str, args: &[&str]) -> String {
    let mut line = String::new();
    // El programa no admite escapes: sólo comillas alrededor.
    if program.contains([' ', '\t']) || program.is_empty() {
        line.push('"');
        line.push_str(program);
        line.push('"');
    } else {
        line.push_str(program);
    }
    for a in args {
        line.push(' ');
        push_arg(&mut line, a);
    }
    line
}

#[cfg_attr(not(windows), allow(dead_code))]
fn push_arg(line: &mut String, arg: &str) {
    let quote = arg.is_empty() || arg.contains([' ', '\t']);
    if quote {
        line.push('"');
    }
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                line.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                backslashes = 0;
            }
            _ => {
                line.extend(std::iter::repeat_n('\\', backslashes));
                backslashes = 0;
            }
        }
        if c != '\\' {
            line.push(c);
        }
    }
    if quote {
        line.extend(std::iter::repeat_n('\\', backslashes * 2));
        line.push('"');
    } else {
        line.extend(std::iter::repeat_n('\\', backslashes));
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::fs::File;
    use std::io::{self, Read};
    use std::os::windows::io::{AsRawHandle, RawHandle};

    type Handle = RawHandle;

    #[repr(C)]
    struct StartupInfoW {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_count_chars: u32,
        y_count_chars: u32,
        fill_attribute: u32,
        flags: u32,
        show_window: u16,
        reserved2_len: u16,
        reserved2: *mut u8,
        std_input: Handle,
        std_output: Handle,
        std_error: Handle,
    }

    #[repr(C)]
    struct StartupInfoExW {
        startup_info: StartupInfoW,
        attribute_list: *mut c_void,
    }

    #[repr(C)]
    struct ProcessInformation {
        process: Handle,
        thread: Handle,
        process_id: u32,
        thread_id: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn InitializeProcThreadAttributeList(
            list: *mut c_void,
            count: u32,
            flags: u32,
            size: *mut usize,
        ) -> i32;
        fn UpdateProcThreadAttribute(
            list: *mut c_void,
            flags: u32,
            attribute: usize,
            value: *const c_void,
            size: usize,
            previous: *mut c_void,
            return_size: *mut usize,
        ) -> i32;
        fn DeleteProcThreadAttributeList(list: *mut c_void);
        #[allow(clippy::too_many_arguments)]
        fn CreateProcessW(
            application: *const u16,
            command_line: *mut u16,
            process_attributes: *const c_void,
            thread_attributes: *const c_void,
            inherit_handles: i32,
            creation_flags: u32,
            environment: *const c_void,
            current_directory: *const u16,
            startup_info: *const StartupInfoW,
            process_information: *mut ProcessInformation,
        ) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
        fn GetCurrentPackageFamilyName(length: *mut u32, name: *mut u16) -> i32;
    }

    const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
    const PROC_THREAD_ATTRIBUTE_DESKTOP_APP_POLICY: usize = 0x0002_0012;
    const PROCESS_CREATION_DESKTOP_APP_BREAKAWAY_ENABLE_PROCESS_TREE: u32 = 0x01;
    const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
    const HANDLE_FLAG_INHERIT: u32 = 0x0000_0001;
    const INFINITE: u32 = u32::MAX;

    /// Familia del paquete (`Nombre_hash`) si el proceso corre empaquetado.
    pub fn package_family_name() -> Option<String> {
        let mut len = 0u32;
        unsafe { GetCurrentPackageFamilyName(&mut len, std::ptr::null_mut()) };
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        if unsafe { GetCurrentPackageFamilyName(&mut len, buf.as_mut_ptr()) } != 0 {
            return None;
        }
        let name = String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]);
        (!name.is_empty()).then_some(name)
    }

    struct Child {
        process: Handle,
    }

    impl Drop for Child {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.process) };
        }
    }

    fn inheritable(h: RawHandle) -> io::Result<Handle> {
        if unsafe { SetHandleInformation(h, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(h)
    }

    /// Crea el proceso fuera del contenedor, sin ventana, con la entrada y la salida en `NUL` y
    /// stderr en `stderr` (o `NUL`). Sólo hereda esos tres identificadores.
    fn create(program: &str, args: &[&str], stderr: Option<RawHandle>) -> io::Result<Child> {
        let null_in = File::open("NUL")?;
        let null_out = std::fs::OpenOptions::new().write(true).open("NUL")?;
        let handles: [Handle; 3] = [
            inheritable(null_in.as_raw_handle())?,
            inheritable(null_out.as_raw_handle())?,
            inheritable(stderr.unwrap_or(null_out.as_raw_handle()))?,
        ];
        // Sin duplicados en la lista de herencia.
        let list: Vec<Handle> = if handles[2] == handles[1] {
            handles[..2].to_vec()
        } else {
            handles.to_vec()
        };

        let mut size = 0usize;
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut size) };
        // `usize` para que la lista quede alineada.
        let mut buf = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
        let attrs = buf.as_mut_ptr() as *mut c_void;
        if unsafe { InitializeProcThreadAttributeList(attrs, 2, 0, &mut size) } == 0 {
            return Err(io::Error::last_os_error());
        }
        struct Attrs(*mut c_void);
        impl Drop for Attrs {
            fn drop(&mut self) {
                unsafe { DeleteProcThreadAttributeList(self.0) };
            }
        }
        let _guard = Attrs(attrs);

        let policy: u32 = PROCESS_CREATION_DESKTOP_APP_BREAKAWAY_ENABLE_PROCESS_TREE;
        let ok = unsafe {
            UpdateProcThreadAttribute(
                attrs,
                0,
                PROC_THREAD_ATTRIBUTE_DESKTOP_APP_POLICY,
                &policy as *const u32 as *const c_void,
                std::mem::size_of::<u32>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let ok = unsafe {
            UpdateProcThreadAttribute(
                attrs,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                list.as_ptr() as *const c_void,
                std::mem::size_of_val(list.as_slice()),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }

        let mut si: StartupInfoExW = unsafe { std::mem::zeroed() };
        si.startup_info.cb = std::mem::size_of::<StartupInfoExW>() as u32;
        si.startup_info.flags = STARTF_USESTDHANDLES;
        si.startup_info.std_input = handles[0];
        si.startup_info.std_output = handles[1];
        si.startup_info.std_error = handles[2];
        si.attribute_list = attrs;

        let mut line: Vec<u16> = super::command_line(program, args)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut pi: ProcessInformation = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            CreateProcessW(
                std::ptr::null(),
                line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
                std::ptr::null(),
                std::ptr::null(),
                &si.startup_info,
                &mut pi,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        unsafe { CloseHandle(pi.thread) };
        Ok(Child {
            process: pi.process,
        })
    }

    /// Ejecuta y espera; devuelve el código de salida y lo que escribió en stderr.
    pub fn run(program: &str, args: &[&str]) -> io::Result<(u32, String)> {
        let (mut reader, writer) = std::io::pipe()?;
        let child = create(program, args, Some(writer.as_raw_handle()))?;
        // Sin nuestra copia del extremo de escritura, la lectura acaba cuando el hijo termina.
        drop(writer);
        let mut err = Vec::new();
        let _ = reader.read_to_end(&mut err);
        let mut code = 0u32;
        unsafe {
            WaitForSingleObject(child.process, INFINITE);
            if GetExitCodeProcess(child.process, &mut code) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok((code, String::from_utf8_lossy(&err).into_owned()))
    }

    /// Abre sin esperar.
    pub fn spawn(program: &str, args: &[&str]) -> io::Result<()> {
        create(program, args, None).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::command_line;

    #[test]
    fn quotes_like_windows_expects() {
        assert_eq!(command_line("reg", &["add", "HKCU\\X"]), r"reg add HKCU\X");
        assert_eq!(
            command_line(r"C:\Program Files\A\a.exe", &["/S"]),
            r#""C:\Program Files\A\a.exe" /S"#
        );
        assert_eq!(command_line("a", &[""]), r#"a """#);
        assert_eq!(
            command_line(
                "powershell",
                &[
                    "-Command",
                    r#"Start-Process -FilePath 'x' -ArgumentList '/i "C:\a b\c.msi"'"#
                ]
            ),
            r#"powershell -Command "Start-Process -FilePath 'x' -ArgumentList '/i \"C:\a b\c.msi\"'""#
        );
        assert_eq!(command_line("a", &[r"C:\dir x\"]), r#"a "C:\dir x\\""#);
        assert_eq!(command_line("a", &[r#"a\"b"#]), r#"a a\\\"b"#);
    }
}
