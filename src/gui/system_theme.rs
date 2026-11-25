//! Tema claro u oscuro del sistema.
//!
//! En Windows y macOS lo informa la ventana (egui lo expone en `Context::system_theme`). En Linux
//! winit no lo sabe (en X11 siempre devuelve `None`), así que se pregunta al escritorio:
//!
//! 1. el portal (`org.freedesktop.appearance color-scheme`), que responde en GNOME, KDE y otros;
//! 2. si no hay portal, el tema GTK de GNOME (`gsettings … gtk-theme`, «Adwaita-dark»…);
//! 3. y si no, el esquema de colores de KDE (`~/.config/kdeglobals`).
//!
//! Se vuelve a consultar cada pocos segundos para seguir un cambio de tema en vivo.

use eframe::egui;

/// Lee el `color-scheme` de la respuesta de `gdbus call … Settings.Read`, p. ej.
/// `(<<uint32 1>>,)`: 1 oscuro, 2 claro, 0 sin preferencia.
pub fn parse_portal(reply: &str) -> Option<bool> {
    let n = reply
        .split(|c: char| !c.is_ascii_digit())
        .rfind(|t| !t.is_empty())?;
    match n {
        "1" => Some(true),
        "2" => Some(false),
        _ => None,
    }
}

/// `gsettings get org.gnome.desktop.interface gtk-theme`: `'Adwaita-dark'` → oscuro.
pub fn parse_gtk_theme(reply: &str) -> Option<bool> {
    let name = reply.trim().trim_matches('\'').trim();
    (!name.is_empty()).then(|| name.to_lowercase().contains("dark"))
}

/// `[General] ColorScheme=BreezeDark` de `kdeglobals`.
pub fn parse_kdeglobals(text: &str) -> Option<bool> {
    let mut general = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            general = line == "[General]";
        } else if general {
            if let Some(v) = line.strip_prefix("ColorScheme=") {
                return Some(v.to_lowercase().contains("dark"));
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn detect() -> Option<bool> {
    use crate::util::output;
    let portal = output(
        "gdbus",
        &[
            "call",
            "--session",
            "--timeout",
            "1",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ],
    );
    if let Some(dark) = portal.as_deref().and_then(parse_portal) {
        return Some(dark);
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    if desktop.contains("kde") {
        let file = crate::util::home()?.join(".config/kdeglobals");
        return parse_kdeglobals(&std::fs::read_to_string(file).ok()?);
    }
    output(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "gtk-theme"],
    )
    .as_deref()
    .and_then(parse_gtk_theme)
}

#[cfg(target_os = "linux")]
mod watch {
    use std::sync::atomic::{AtomicU8, Ordering};

    /// 0: sin saber, 1: claro, 2: oscuro.
    static LINUX: AtomicU8 = AtomicU8::new(0);

    pub fn get() -> Option<bool> {
        match LINUX.load(Ordering::Relaxed) {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    }

    /// Consulta ya (para el primer fotograma) y luego cada 3 s en segundo plano.
    pub fn start(ctx: eframe::egui::Context) {
        let code = |d: Option<bool>| match d {
            Some(false) => 1,
            Some(true) => 2,
            None => 0,
        };
        LINUX.store(code(super::detect()), Ordering::Relaxed);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let new = code(super::detect());
            if LINUX.swap(new, Ordering::Relaxed) != new {
                ctx.request_repaint();
            }
        });
    }
}

/// Empieza a seguir el tema del sistema (en Linux; en otros sistemas no hace falta).
pub fn start(_ctx: &egui::Context) {
    #[cfg(target_os = "linux")]
    watch::start(_ctx.clone());
}

/// ¿El sistema usa tema oscuro? `None` si no se sabe.
pub fn is_dark(ctx: &egui::Context) -> Option<bool> {
    #[cfg(target_os = "linux")]
    if let Some(dark) = watch::get() {
        return Some(dark);
    }
    ctx.system_theme().map(|t| t == egui::Theme::Dark)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal() {
        assert_eq!(parse_portal("(<<uint32 1>>,)"), Some(true));
        assert_eq!(parse_portal("(<<uint32 2>>,)\n"), Some(false));
        assert_eq!(parse_portal("(<<uint32 0>>,)"), None);
        assert_eq!(parse_portal(""), None);
    }

    #[test]
    fn gtk_y_kde() {
        assert_eq!(parse_gtk_theme("'Adwaita-dark'\n"), Some(true));
        assert_eq!(parse_gtk_theme("'Yaru'"), Some(false));
        assert_eq!(parse_gtk_theme("''"), None);
        let kde = "[Colors:View]\nColorScheme=Nada\n[General]\nColorScheme=BreezeDark\n";
        assert_eq!(parse_kdeglobals(kde), Some(true));
        assert_eq!(
            parse_kdeglobals("[General]\nColorScheme=BreezeLight"),
            Some(false)
        );
        assert_eq!(parse_kdeglobals("[General]\nfont=x"), None);
    }
}
