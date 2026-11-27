//! Capturas de pantalla de cada app para su ficha.
//!
//! Van siempre en el mismo lugar del repo de cada app (el de sus releases), en la rama por
//! defecto:
//!
//! ```text
//! docs/screenshots/1.png
//! docs/screenshots/2.png        … hasta 6, en orden y sin saltos
//! docs/screenshots/captions.txt (opcional) un pie de foto por línea, en el mismo orden
//! ```
//!
//! Se leen de `raw.githubusercontent.com` (no gastan consultas de la API) y se guardan en
//! `<datos>/shots/<app>/` para verlas sin conexión.

use crate::github;
use std::path::PathBuf;

/// Carpeta de las capturas dentro del repo de cada app.
pub const DIR: &str = "docs/screenshots";
/// Como mucho tantas capturas por app, y de este tamaño.
const MAX_SHOTS: usize = 6;
const MAX_PNG: u64 = 8 * 1024 * 1024;
const MAX_CAPTIONS: u64 = 16 * 1024;
const CAPTIONS: &str = "captions.txt";

/// Una captura: pie de foto y el PNG sin decodificar.
pub struct Shot {
    pub caption: String,
    pub png: Vec<u8>,
}

fn cache_dir(id: &str) -> PathBuf {
    crate::util::data_dir().join("shots").join(id)
}

fn raw_url(repo: &str, file: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo}/HEAD/{DIR}/{file}")
}

/// Pies de foto de `captions.txt`: uno por línea; las vacías dejan la captura sin pie.
fn parse_captions(text: &str) -> Vec<String> {
    text.lines().map(|l| l.trim().to_string()).collect()
}

/// Capturas de la app `id` (repo `repo`): de GitHub si hay conexión (y se actualiza la
/// caché) o de la caché. Lista vacía si la app no publica ninguna.
pub fn load(id: &str, repo: &str, token: Option<&str>) -> Result<Vec<Shot>, String> {
    match fetch(repo, token) {
        Ok(shots) => {
            save_cache(id, &shots);
            Ok(shots)
        }
        Err(e) => read_cache(id).ok_or(e),
    }
}

fn fetch(repo: &str, token: Option<&str>) -> Result<Vec<Shot>, String> {
    let mut pngs = Vec::new();
    for n in 1..=MAX_SHOTS {
        match github::fetch_bytes(&raw_url(repo, &format!("{n}.png")), token, MAX_PNG)? {
            Some(png) => pngs.push(png),
            None => break,
        }
    }
    if pngs.is_empty() {
        return Ok(Vec::new());
    }
    let captions = github::fetch_bytes(&raw_url(repo, CAPTIONS), token, MAX_CAPTIONS)
        .ok()
        .flatten()
        .map(|b| parse_captions(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    Ok(pngs
        .into_iter()
        .enumerate()
        .map(|(k, png)| Shot {
            caption: captions.get(k).cloned().unwrap_or_default(),
            png,
        })
        .collect())
}

fn save_cache(id: &str, shots: &[Shot]) {
    let dir = cache_dir(id);
    let _ = std::fs::remove_dir_all(&dir);
    if shots.is_empty() || std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (k, s) in shots.iter().enumerate() {
        let _ = std::fs::write(dir.join(format!("{}.png", k + 1)), &s.png);
    }
    let captions: Vec<&str> = shots.iter().map(|s| s.caption.as_str()).collect();
    let _ = std::fs::write(dir.join(CAPTIONS), captions.join("\n"));
}

fn read_cache(id: &str) -> Option<Vec<Shot>> {
    let dir = cache_dir(id);
    let captions = std::fs::read_to_string(dir.join(CAPTIONS))
        .map(|t| parse_captions(&t))
        .unwrap_or_default();
    let shots: Vec<Shot> = (1..=MAX_SHOTS)
        .map_while(|n| std::fs::read(dir.join(format!("{n}.png"))).ok())
        .enumerate()
        .map(|(k, png)| Shot {
            caption: captions.get(k).cloned().unwrap_or_default(),
            png,
        })
        .collect();
    (!shots.is_empty()).then_some(shots)
}

#[cfg(test)]
mod tests {
    use super::{parse_captions, raw_url};

    #[test]
    fn ruta_fija_de_las_capturas() {
        assert_eq!(
            raw_url("ellaguno/iureditor", "1.png"),
            "https://raw.githubusercontent.com/ellaguno/iureditor/HEAD/docs/screenshots/1.png"
        );
    }

    #[test]
    fn pies_de_foto() {
        let c = parse_captions("Edición con vista previa\n\n  Exportar a Word  \r\n");
        assert_eq!(c, ["Edición con vista previa", "", "Exportar a Word"]);
    }
}
