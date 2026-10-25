//! Consulta de releases en GitHub y descarga de instaladores.

use serde::Deserialize;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
    /// URL de la API; con token permite bajar assets de repos privados.
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

impl Release {
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }
    pub fn date(&self) -> &str {
        self.published_at
            .as_deref()
            .map(|d| d.get(..10).unwrap_or(d))
            .unwrap_or("")
    }
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent(concat!("iureapps/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Token de GitHub para repos privados: variable de entorno o el guardado en el estado.
pub fn token(saved: Option<&str>) -> Option<String> {
    ["IUREAPPS_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|v| std::env::var(v).ok())
        .or_else(|| saved.map(str::to_string))
        .filter(|t| !t.trim().is_empty())
}

pub fn latest_release(repo: &str, token: Option<&str>) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut req = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token {
        req = req.header("Authorization", &format!("Bearer {t}"));
    }
    match req.call() {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<Release>()
            .map_err(|e| e.to_string()),
        Err(ureq::Error::StatusCode(404)) => Err(if token.is_some() {
            "sin versiones publicadas".into()
        } else {
            "sin versiones publicadas o repositorio privado (configura un token de GitHub)".into()
        }),
        Err(ureq::Error::StatusCode(403 | 429)) => latest_from_manifest(repo).map_err(|_| {
            "GitHub limitó las consultas anónimas; prueba en un rato o configura un token".into()
        }),
        Err(e) => latest_from_manifest(repo).map_err(|_| format!("sin conexión con GitHub: {e}")),
    }
}

/// Alternativa sin la API (que limita a 60 consultas por hora): el `latest.json` del
/// actualizador de Tauri. Sólo lista los instaladores que se actualizan solos.
fn latest_from_manifest(repo: &str) -> Result<Release, String> {
    #[derive(Deserialize)]
    struct Platform {
        url: String,
    }
    #[derive(Deserialize)]
    struct Manifest {
        version: String,
        #[serde(default)]
        notes: Option<String>,
        #[serde(default)]
        pub_date: Option<String>,
        platforms: std::collections::BTreeMap<String, Platform>,
    }
    let url = format!("https://github.com/{repo}/releases/latest/download/latest.json");
    let m: Manifest = agent()
        .get(&url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_json()
        .map_err(|e| e.to_string())?;
    let mut assets: Vec<Asset> = Vec::new();
    for p in m.platforms.values() {
        for u in [p.url.clone(), format!("{}.sig", p.url)] {
            let name = u.rsplit('/').next().unwrap_or_default().to_string();
            if !assets.iter().any(|a| a.name == name) {
                assets.push(Asset {
                    name,
                    size: 0,
                    browser_download_url: u,
                    url: String::new(),
                });
            }
        }
    }
    let tag = format!("v{}", m.version.trim_start_matches('v'));
    Ok(Release {
        html_url: format!("https://github.com/{repo}/releases/tag/{tag}"),
        tag_name: tag,
        body: m.notes,
        published_at: m.pub_date,
        assets,
    })
}

/// Petición de un asset. Con token (y si se conoce la URL de la API) se usa la API, que
/// sirve también para repos privados; ureq no reenvía la cabecera Authorization al
/// redirigir al almacenamiento de GitHub.
fn get_asset(
    asset: &Asset,
    token: Option<&str>,
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    let a = agent();
    match token {
        Some(t) if !asset.url.is_empty() => a
            .get(&asset.url)
            .header("Accept", "application/octet-stream")
            .header("Authorization", &format!("Bearer {t}"))
            .call(),
        _ => a.get(&asset.browser_download_url).call(),
    }
}

/// Descarga `asset` a `dest`, informando el avance (0.0–1.0).
pub fn download(
    asset: &Asset,
    token: Option<&str>,
    dest: &Path,
    mut progress: impl FnMut(f32),
) -> Result<(), String> {
    let resp =
        get_asset(asset, token).map_err(|e| format!("no se pudo descargar {}: {e}", asset.name))?;

    let total = resp.body().content_length().unwrap_or(asset.size).max(1);
    let mut reader = resp.into_body().into_reader();
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("descarga interrumpida: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done as f32 / total as f32);
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
}

/// Descarga un archivo pequeño (p. ej. la firma `.sig`) a memoria.
pub fn fetch_text(asset: &Asset, token: Option<&str>) -> Result<String, String> {
    let resp = get_asset(asset, token);
    resp.map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}
