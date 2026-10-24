//! iureapps: lanzador e instalador ligero de las apps de escritorio de Iurefficient.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod catalog;
mod cli;
mod github;
mod gui;
mod installer;
mod plan;
mod state;
mod system;
mod util;
mod verify;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        attach_console();
        std::process::exit(cli::run(&args));
    }
    if let Err(e) = gui::run() {
        eprintln!("No se pudo abrir la ventana: {e}\nUsa `iureapps ayuda` para el modo de línea de comandos.");
        std::process::exit(1);
    }
}

/// En Windows la app no tiene consola propia: para el modo CLI se usa la de quien la llamó.
fn attach_console() {
    #[cfg(windows)]
    {
        extern "system" {
            fn AttachConsole(process_id: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
