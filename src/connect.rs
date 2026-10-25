//! Cuenta de Iurefficient compartida por todas las apps, con `iurefficient-connect`.
//!
//! El lanzador hace lo mismo que cada app al iniciar sesión: guarda la sesión REST en
//! el llavero del sistema (entrada compartida), crea una contraseña de aplicación
//! WebDAV si no hay ninguna y deja la cuenta como «cuenta activa». Así IureDav,
//! IureTranscribe, IureOCR e iureditor arrancan ya conectadas.

use iurefficient_connect::account::{self, ActiveAccount};
use iurefficient_connect::rest::{Login, Session, User};
use iurefficient_connect::{api, lang, secrets, user_agent, Account};

const APP_NAME: &str = "Iurefficient Apps";

/// Lo que se sabe de la cuenta sin conectarse a la instancia.
#[derive(Clone, Debug)]
pub struct Saved {
    pub domain: String,
    pub email: String,
    /// App que dejó la cuenta activa.
    pub app: String,
    pub web_url: String,
    pub has_session: bool,
    pub has_webdav: bool,
}

/// Resultado de iniciar sesión.
#[derive(Clone, Debug)]
pub enum LoginOutcome {
    Ok {
        name: String,
        created_webdav: bool,
    },
    /// La cuenta tiene verificación en dos pasos.
    TotpRequired {
        totp_token: String,
    },
}

pub fn init() {
    // Los mensajes del conector salen en el idioma de la interfaz del lanzador.
    lang::set(lang::resolve("es"));
}

fn ua() -> String {
    user_agent(APP_NAME, env!("CARGO_PKG_VERSION"))
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("no se pudo iniciar la conexión: {e}"))
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

/// Cuenta activa guardada por cualquier app (o por el lanzador).
pub fn saved() -> Option<Saved> {
    let a: ActiveAccount = account::active()?;
    let acc = a.account().ok()?;
    let has = |k| matches!(secrets::leer(&acc, k), Ok(Some(s)) if !s.trim().is_empty());
    Some(Saved {
        domain: a.domain.clone(),
        email: a.email.clone(),
        app: a.app.clone(),
        web_url: acc.web_url(),
        has_session: has(secrets::Kind::Session),
        has_webdav: has(secrets::Kind::WebDav),
    })
}

/// Comprueba con la instancia que la sesión guardada sigue valiendo; devuelve el nombre.
pub fn verify(saved: &Saved) -> Result<String, String> {
    let acc = Account::new(&saved.domain, &saved.email).map_err(err)?;
    runtime()?.block_on(async {
        match Session::restore_shared(acc, &ua()).await.map_err(err)? {
            Some((sess, user)) => {
                sess.push_shared();
                Ok(display_name(&user))
            }
            None => Err("no hay sesión guardada".into()),
        }
    })
}

fn display_name(u: &User) -> String {
    u.name
        .clone()
        .or_else(|| {
            u.extra
                .get("full_name")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| u.email.clone())
}

fn host_label() -> String {
    ["HOSTNAME", "COMPUTERNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok())
        .or_else(|| crate::util::output("hostname", &[]))
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "este equipo".into())
}

/// Inicia sesión con correo y contraseña; si hace falta, con el código TOTP
/// (`totp = Some((token, código))`).
pub fn login(
    domain: &str,
    email: &str,
    password: &str,
    totp: Option<(&str, &str)>,
) -> Result<LoginOutcome, String> {
    let acc = Account::new(domain, email).map_err(err)?;
    runtime()?.block_on(async {
        let sess = Session::new(acc.clone(), &ua()).map_err(err)?;
        let user = match totp {
            Some((token, code)) => sess.verify_totp(token, code.trim()).await.map_err(err)?,
            None => match sess.login(password).await.map_err(err)? {
                Login::Ok(u) => u,
                Login::TotpRequired { totp_token } => {
                    return Ok(LoginOutcome::TotpRequired { totp_token })
                }
            },
        };
        let json = serde_json::to_string(&sess.export()).map_err(|e| e.to_string())?;
        secrets::guardar(&acc, secrets::Kind::Session, &json).map_err(err)?;
        account::set_active(&acc, APP_NAME).map_err(err)?;
        // Contraseña de aplicación WebDAV para IureDav, IureTranscribe e IureOCR.
        let has_webdav = matches!(secrets::leer(&acc, secrets::Kind::WebDav), Ok(Some(s)) if !s.trim().is_empty());
        let mut created_webdav = false;
        if !has_webdav {
            let label = format!("{APP_NAME} en {}", host_label());
            let created = api::create_webdav_token(&sess, &label, None).await.map_err(err)?;
            secrets::guardar(&acc, secrets::Kind::WebDav, &created.secret).map_err(err)?;
            created_webdav = true;
        }
        Ok(LoginOutcome::Ok { name: display_name(&user), created_webdav })
    })
}

/// Cierra la sesión en la instancia y en este equipo. La contraseña WebDAV se conserva
/// (IureDav la usa para la unidad montada), como hacen las apps.
pub fn logout(saved: &Saved) -> Result<(), String> {
    let acc = Account::new(&saved.domain, &saved.email).map_err(err)?;
    let rt = runtime()?;
    rt.block_on(async {
        if let Ok(Some((sess, _))) = Session::restore_shared(acc.clone(), &ua()).await {
            let _ = sess.logout().await;
        }
    });
    secrets::borrar(&acc, secrets::Kind::Session).map_err(err)?;
    account::clear_active(&acc).map_err(err)
}
