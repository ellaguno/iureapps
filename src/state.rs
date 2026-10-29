//! Estado persistente: qué instaló el lanzador, preferencias por app y token.

use crate::catalog::{Format, Variant};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Managed {
    pub version: String,
    pub variant: Variant,
    pub format: Format,
    /// AppImage o .app instalados por el lanzador (para .deb/.rpm/.exe se detecta aparte).
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pref {
    pub variant: Option<Variant>,
    pub format: Option<Format>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub installs: BTreeMap<String, Managed>,
    #[serde(default)]
    pub prefs: BTreeMap<String, Pref>,
    #[serde(default)]
    pub github_token: Option<String>,
    /// Vista de la ventana: mosaico (por defecto) o tabla.
    #[serde(default)]
    pub table_view: bool,
}

fn path() -> PathBuf {
    crate::util::data_dir().join("state.json")
}

impl State {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&p, text);
            #[cfg(unix)]
            {
                // Puede contener el token de GitHub.
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

/// Compara versiones `1.2.3` (ignora una `v` inicial y sufijos como `-1` de Debian). Un sufijo
/// que empieza con letra es de versión preliminar (semver): `0.1.0-alpha.2` > `0.1.0-alpha.1`
/// y `0.1.0` > `0.1.0-alpha.2`.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    core_cmp(a, b).then_with(|| pre_cmp(pre(a), pre(b)))
}

/// Identificadores de la parte preliminar (`alpha.1` → `["alpha", "1"]`), si la hay.
fn pre(v: &str) -> Option<Vec<&str>> {
    let v = v.trim().trim_start_matches('v');
    let rest = v.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
    let rest = rest.strip_prefix('-')?;
    // Sin letra al principio es la revisión de Debian/RPM (`-1`), no una preliminar.
    if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let rest = rest.split('+').next().unwrap_or(rest);
    Some(rest.split(['.', '-']).filter(|s| !s.is_empty()).collect())
}

fn pre_cmp(a: Option<Vec<&str>>, b: Option<Vec<&str>>) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a, b) {
        (None, None) => Equal,
        (None, Some(_)) => Greater,
        (Some(_), None) => Less,
        (Some(a), Some(b)) => {
            for (x, y) in a.iter().zip(b.iter()) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Less,
                    (Err(_), Ok(_)) => Greater,
                    _ => x.cmp(y),
                };
                if o != Equal {
                    return o;
                }
            }
            a.len().cmp(&b.len())
        }
    }
}

fn core_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(v: &str) -> Vec<u64> {
        let v = v.trim().trim_start_matches('v');
        let core: String = v
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        core.split('.')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap_or(0))
            .collect()
    }
    let (mut a, mut b) = (parts(a), parts(b));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a.cmp(&b)
}

#[cfg(test)]
mod tests {
    use super::version_cmp;
    use std::cmp::Ordering::*;

    #[test]
    fn compares_versions() {
        assert_eq!(version_cmp("v1.13.0", "1.13.0"), Equal);
        assert_eq!(version_cmp("0.2.1-1", "0.2.1"), Equal);
        assert_eq!(version_cmp("0.8.2", "0.10.0"), Less);
        assert_eq!(version_cmp("1.0", "0.9.9"), Greater);
    }

    #[test]
    fn compares_prereleases() {
        assert_eq!(version_cmp("0.1.0-alpha.1", "0.1.0-alpha.2"), Less);
        assert_eq!(version_cmp("v0.1.0-alpha.2", "0.1.0-alpha.2"), Equal);
        assert_eq!(version_cmp("0.1.0-alpha.10", "0.1.0-alpha.9"), Greater);
        assert_eq!(version_cmp("0.1.0-beta.1", "0.1.0-alpha.3"), Greater);
        assert_eq!(version_cmp("0.1.0", "0.1.0-alpha.3"), Greater);
        assert_eq!(version_cmp("0.1.0-alpha.3", "0.1.1"), Less);
        assert_eq!(version_cmp("0.1.0-alpha", "0.1.0-alpha.1"), Less);
        // La revisión de Debian sigue sin contar.
        assert_eq!(version_cmp("0.2.1-1", "0.2.1"), Equal);
    }
}
