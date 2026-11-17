//! Capturas de pantalla de cada app para su ficha.
//!
//! Cada app las anuncia en un `iureapps.json` en la raíz de su repo (rama por defecto):
//!
//! ```json
//! { "screenshots": [
//!     { "url": "screenshots/editor.png", "caption": "Edición con vista previa" } ] }
//! ```
//!
//! `url` es una ruta dentro del repo o una URL `https://` completa; sólo PNG. Lo descargado
//! se guarda en `<datos>/shots/<app>/` y se usa sin conexión.

use crate::catalog::AppDef;
use crate::github;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Como mucho tantas capturas por app, y de este tamaño.
const MAX_SHOTS: usize = 6;
const MAX_PNG: u64 = 8 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;

#[derive(Deserialize, Serialize)]
struct Manifest {
    #[serde(default)]
    screenshots: Vec<Entry>,
}

#[derive(Deserialize, Serialize)]
struct Entry {
    url: String,
    #[serde(default)]
    caption: String,
}

/// Una captura: pie de foto y el PNG sin decodificar.
pub struct Shot {
    pub caption: String,
    pub png: Vec<u8>,
}

fn cache_dir(app: &AppDef) -> PathBuf {
    crate::util::data_dir().join("shots").join(app.id)
}

fn raw_url(repo: &str, path: &str) -> Option<String> {
    if path.starts_with("https://") {
        return Some(path.to_string());
    }
    if path.contains("://") || path.split('/').any(|p| p == "..") {
        return None;
    }
    let path = path.trim_start_matches("./").trim_start_matches('/');
    Some(format!(
        "https://raw.githubusercontent.com/{repo}/HEAD/{path}"
    ))
}

/// Capturas de `app`: de GitHub si hay conexión (y se actualiza la caché) o de la caché.
/// Lista vacía si la app no publica ninguna.
pub fn load(app: &AppDef, token: Option<&str>) -> Result<Vec<Shot>, String> {
    match fetch(app, token) {
        Ok(shots) => {
            save_cache(app, &shots);
            Ok(shots)
        }
        Err(e) => match read_cache(app) {
            Some(shots) => Ok(shots),
            None => Err(e),
        },
    }
}

fn fetch(app: &AppDef, token: Option<&str>) -> Result<Vec<Shot>, String> {
    let url = raw_url(app.repo, "iureapps.json").unwrap_or_default();
    let Some(bytes) = github::fetch_bytes(&url, token, MAX_MANIFEST)? else {
        return Ok(Vec::new());
    };
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("iureapps.json no es válido: {e}"))?;
    let mut shots = Vec::new();
    for entry in manifest.screenshots.into_iter().take(MAX_SHOTS) {
        let Some(url) = raw_url(app.repo, &entry.url) else {
            continue;
        };
        if let Some(png) = github::fetch_bytes(&url, token, MAX_PNG)? {
            shots.push(Shot {
                caption: entry.caption,
                png,
            });
        }
    }
    Ok(shots)
}

fn save_cache(app: &AppDef, shots: &[Shot]) {
    let dir = cache_dir(app);
    let _ = std::fs::remove_dir_all(&dir);
    if shots.is_empty() || std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut manifest = Manifest {
        screenshots: Vec::new(),
    };
    for (k, s) in shots.iter().enumerate() {
        let name = format!("{k}.png");
        if std::fs::write(dir.join(&name), &s.png).is_ok() {
            manifest.screenshots.push(Entry {
                url: name,
                caption: s.caption.clone(),
            });
        }
    }
    if let Ok(json) = serde_json::to_vec_pretty(&manifest) {
        let _ = std::fs::write(dir.join("iureapps.json"), json);
    }
}

fn read_cache(app: &AppDef) -> Option<Vec<Shot>> {
    let dir = cache_dir(app);
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(dir.join("iureapps.json")).ok()?).ok()?;
    Some(
        manifest
            .screenshots
            .into_iter()
            .filter_map(|e| {
                let png = std::fs::read(dir.join(&e.url)).ok()?;
                Some(Shot {
                    caption: e.caption,
                    png,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::raw_url;

    #[test]
    fn rutas_de_capturas() {
        assert_eq!(
            raw_url("ellaguno/iureditor", "screenshots/a.png").as_deref(),
            Some("https://raw.githubusercontent.com/ellaguno/iureditor/HEAD/screenshots/a.png")
        );
        assert_eq!(
            raw_url("ellaguno/iureditor", "./a.png").as_deref(),
            Some("https://raw.githubusercontent.com/ellaguno/iureditor/HEAD/a.png")
        );
        assert_eq!(
            raw_url("x/y", "https://ejemplo.com/a.png").as_deref(),
            Some("https://ejemplo.com/a.png")
        );
        assert_eq!(raw_url("x/y", "http://ejemplo.com/a.png"), None);
        assert_eq!(raw_url("x/y", "../otro/a.png"), None);
    }
}
