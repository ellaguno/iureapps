//! Una sola ventana del lanzador: la primera instancia escucha en un puerto local (anotado en
//! `<datos>/instance.port`); si se abre otra, le pide que se muestre y termina.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

const ASK: &[u8] = b"iureapps:mostrar\n";
const ANSWER: &str = "iureapps:ok";

fn port_file() -> PathBuf {
    crate::util::data_dir().join("instance.port")
}

/// Si ya hay un lanzador abierto, le pide que se muestre y devuelve `true`.
pub fn wake_existing() -> bool {
    wake_at(&port_file())
}

fn wake_at(file: &std::path::Path) -> bool {
    let Some(port) = std::fs::read_to_string(file)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
    else {
        return false;
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(400)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(800)));
    if s.write_all(ASK).is_err() {
        return false;
    }
    // Otro programa podría haber tomado el puerto: sólo cuenta si contesta el lanzador.
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).is_ok() && line.trim() == ANSWER
}

/// Empieza a escuchar; `on_wake` se llama cada vez que otra instancia pide mostrarse.
pub fn listen(on_wake: impl Fn() + Send + 'static) {
    listen_at(port_file(), on_wake)
}

fn listen_at(file: PathBuf, on_wake: impl Fn() + Send + 'static) {
    let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else {
        return;
    };
    let Ok(addr) = listener.local_addr() else {
        return;
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, addr.port().to_string());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() && line.as_bytes() == ASK {
                let _ = (&stream).write_all(format!("{ANSWER}\n").as_bytes());
                on_wake();
            }
        }
    });
}

/// Deja de anunciarse (antes de reiniciar tras una actualización).
pub fn release() {
    let _ = std::fs::remove_file(port_file());
}

#[cfg(test)]
mod tests {
    #[test]
    fn despierta_a_la_primera() {
        let dir = std::env::temp_dir().join(format!("iureapps-inst-{}", std::process::id()));
        let file = dir.join("instance.port");
        assert!(!super::wake_at(&file));
        let (tx, rx) = std::sync::mpsc::channel();
        super::listen_at(file.clone(), move || {
            let _ = tx.send(());
        });
        assert!(super::wake_at(&file));
        assert!(rx.recv_timeout(std::time::Duration::from_secs(2)).is_ok());
        // Un puerto anotado en el que no escucha el lanzador no cuenta.
        std::fs::write(&file, "1").unwrap();
        assert!(!super::wake_at(&file));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
