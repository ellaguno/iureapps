//! Decide qué hacer con cada app: estado (instalada, actualizable…) y qué variante/formato usar.

use crate::catalog::{AppDef, Format, Variant};
use crate::github::Release;
use crate::installer::Installed;
use crate::state::{version_cmp, Pref};
use crate::system::SystemInfo;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Unsupported,
    Checking,
    NotInstalled,
    UpToDate,
    UpdateAvailable,
    /// Instalada, pero no se pudo consultar la última versión.
    InstalledUnknown,
    /// No hay release que instalar (o no se pudo consultar).
    Unavailable,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Unsupported => "No disponible para este sistema",
            Status::Checking => "Consultando…",
            Status::NotInstalled => "No instalada",
            Status::UpToDate => "Al día",
            Status::UpdateAvailable => "Actualización disponible",
            Status::InstalledUnknown => "Instalada",
            Status::Unavailable => "Sin versión publicada",
        }
    }
}

pub fn status(
    app: &AppDef,
    sys: &SystemInfo,
    release: Option<&Result<Release, String>>,
    installed: Option<&Installed>,
) -> Status {
    if !app.supports(sys) && installed.is_none() {
        return Status::Unsupported;
    }
    match (installed, release) {
        (_, None) => Status::Checking,
        (Some(i), Some(Ok(r))) => match &i.version {
            Some(v) if version_cmp(v, r.version()).is_lt() => Status::UpdateAvailable,
            _ => Status::UpToDate,
        },
        (Some(_), Some(Err(_))) => Status::InstalledUnknown,
        (None, Some(Ok(r))) => {
            if app.formats(sys).iter().any(|f| {
                app.variants(sys)
                    .iter()
                    .any(|v| app.select_asset(r, sys, *v, *f).is_some())
            }) {
                Status::NotInstalled
            } else {
                Status::Unsupported
            }
        }
        (None, Some(Err(_))) => Status::Unavailable,
    }
}

/// Variante a instalar: la elegida, la instalada, la preferida o la recomendada.
pub fn variant(
    app: &AppDef,
    sys: &SystemInfo,
    installed: Option<&Installed>,
    pref: Option<&Pref>,
) -> Variant {
    pref.and_then(|p| p.variant)
        .or_else(|| installed.and_then(|i| i.variant))
        .filter(|v| app.variants(sys).contains(v))
        .unwrap_or_else(|| app.recommend_variant(sys).0)
}

/// Formato a instalar para esa variante: el preferido o el instalado si existen en la
/// release; si no, el primero disponible (p. ej. CUDA en Linux sólo viene en .deb).
pub fn format(
    app: &AppDef,
    sys: &SystemInfo,
    release: &Release,
    variant: Variant,
    installed: Option<&Installed>,
    pref: Option<&Pref>,
) -> Option<Format> {
    [pref.and_then(|p| p.format), installed.map(|i| i.format)]
        .into_iter()
        .flatten()
        .find(|f| {
            app.formats(sys).contains(f) && app.select_asset(release, sys, variant, *f).is_some()
        })
        .or_else(|| app.best_format(release, sys, variant))
}
