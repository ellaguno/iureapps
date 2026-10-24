//! Interfaz gráfica (egui): mosaico de apps con su icono, estado y acciones.

mod theme;
mod view;

use crate::catalog::{AppDef, APPS};
use crate::connect;
use crate::github::{self, Release};
use crate::installer::{self, Installed};
use crate::plan::{self, Status};
use crate::state::{version_cmp, Managed, State};
use crate::system::SystemInfo;
use eframe::egui::{self, RichText};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

const SELF_REPO: &str = "ellaguno/iureapps";

enum Msg {
    System(Box<SystemInfo>),
    Detected(usize, Option<Installed>),
    Release(usize, Result<Release, String>),
    SelfRelease(Release),
    Progress(usize, f32),
    Log(String),
    Done(usize, Result<Option<Managed>, String>),
    AccountLoaded(Option<connect::Saved>),
    AccountVerified(Result<String, String>),
    LoginDone(Result<connect::LoginOutcome, String>),
    LogoutDone(Result<(), String>),
}

/// Estado de la cuenta de Iurefficient compartida.
#[derive(Default)]
struct AccountState {
    loaded: bool,
    saved: Option<connect::Saved>,
    /// Nombre devuelto por la instancia al comprobar la sesión.
    name: Option<String>,
    checking: bool,
    /// La sesión guardada ya no vale.
    expired: bool,
    busy: bool,
    error: Option<String>,
    /// Nombre recién devuelto al iniciar sesión: no hace falta volver a comprobarla.
    just_logged: Option<String>,
}

/// Formulario de inicio de sesión.
#[derive(Default)]
struct LoginForm {
    domain: String,
    email: String,
    password: String,
    totp_token: Option<String>,
    code: String,
    busy: bool,
    error: Option<String>,
    /// Ya se puso el cursor en el primer campo vacío.
    focused: bool,
}

#[derive(Default)]
struct Row {
    release: Option<Result<Release, String>>,
    installed: Option<Installed>,
    detected: bool,
    busy: Option<String>,
    progress: Option<f32>,
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Installed,
    Updates,
    NotInstalled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Grid,
    Table,
}

pub struct LauncherApp {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    sys: Option<SystemInfo>,
    rows: Vec<Row>,
    state: State,
    log: Vec<String>,
    self_update: Option<Release>,
    show_settings: bool,
    show_system: bool,
    show_log: bool,
    update_dismissed: bool,
    confirm_uninstall: Option<usize>,
    token_input: String,
    filter: Filter,
    view: ViewMode,
    search: String,
    icons: theme::Icons,
    last_checked: Option<Instant>,
    account: AccountState,
    login: Option<LoginForm>,
    confirm_logout: bool,
}

pub fn run() -> Result<(), String> {
    let mut viewport = egui::ViewportBuilder::default();
    if let Some(icon) = theme::window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport: viewport
            .with_title("Iurefficient Apps")
            // Igual que el .desktop, para que el escritorio asocie la ventana con su icono.
            .with_app_id("iureapps")
            .with_inner_size(window_size())
            .with_min_inner_size([480.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Iurefficient Apps",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc.egui_ctx.clone())))),
    )
    .map_err(|e| e.to_string())
}

/// Tamaño inicial; `IUREAPPS_WINDOW_SIZE=1280x900` lo cambia (útil para capturas).
fn window_size() -> [f32; 2] {
    std::env::var("IUREAPPS_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
        })
        .unwrap_or([1120.0, 780.0])
}

impl LauncherApp {
    fn new(ctx: egui::Context) -> Self {
        theme::install(&ctx);
        let (tx, rx) = channel();
        let state = State::load();
        let icons = theme::Icons::load(&ctx);
        let view = if state.table_view {
            ViewMode::Table
        } else {
            ViewMode::Grid
        };
        let mut app = LauncherApp {
            tx,
            rx,
            ctx,
            sys: None,
            rows: APPS.iter().map(|_| Row::default()).collect(),
            token_input: state.github_token.clone().unwrap_or_default(),
            state,
            log: Vec::new(),
            self_update: None,
            show_settings: false,
            show_system: false,
            show_log: false,
            update_dismissed: false,
            confirm_uninstall: None,
            filter: Filter::All,
            view,
            search: String::new(),
            icons,
            last_checked: None,
            account: AccountState::default(),
            login: None,
            confirm_logout: false,
        };
        app.spawn_system();
        app.load_account();
        app.refresh_releases();
        app
    }

    fn token(&self) -> Option<String> {
        github::token(self.state.github_token.as_deref())
    }

    fn send(tx: &Sender<Msg>, ctx: &egui::Context, msg: Msg) {
        let _ = tx.send(msg);
        ctx.request_repaint();
    }

    fn spawn_system(&self) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let managed = self.state.installs.clone();
        std::thread::spawn(move || {
            let sys = SystemInfo::detect();
            for (i, app) in APPS.iter().enumerate() {
                let inst = installer::detect(app, &sys, managed.get(app.id));
                Self::send(&tx, &ctx, Msg::Detected(i, inst));
            }
            Self::send(&tx, &ctx, Msg::System(Box::new(sys)));
        });
    }

    fn redetect(&self, i: usize) {
        let Some(sys) = self.sys.clone() else { return };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let managed = self.state.installs.get(APPS[i].id).cloned();
        std::thread::spawn(move || {
            let inst = installer::detect(&APPS[i], &sys, managed.as_ref());
            Self::send(&tx, &ctx, Msg::Detected(i, inst));
        });
    }

    fn refresh_releases(&mut self) {
        let token = self.token();
        for (i, app) in APPS.iter().enumerate() {
            self.rows[i].release = None;
            let (tx, ctx, token) = (self.tx.clone(), self.ctx.clone(), token.clone());
            std::thread::spawn(move || {
                let r = github::latest_release(app.repo, token.as_deref());
                Self::send(&tx, &ctx, Msg::Release(i, r));
            });
        }
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            if let Ok(r) = github::latest_release(SELF_REPO, None) {
                Self::send(&tx, &ctx, Msg::SelfRelease(r));
            }
        });
    }

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::System(sys) => self.sys = Some(*sys),
                Msg::Detected(i, inst) => {
                    self.rows[i].installed = inst;
                    self.rows[i].detected = true;
                }
                Msg::Release(i, r) => {
                    if let Err(e) = &r {
                        self.log.push(format!("{}: {e}", APPS[i].name));
                    }
                    self.rows[i].release = Some(r);
                    if self.rows.iter().all(|r| r.release.is_some()) {
                        self.last_checked = Some(Instant::now());
                    }
                }
                Msg::SelfRelease(r) => {
                    if version_cmp(r.version(), env!("CARGO_PKG_VERSION")).is_gt() {
                        self.self_update = Some(r);
                    }
                }
                Msg::Progress(i, p) => self.rows[i].progress = Some(p),
                Msg::Log(m) => self.log.push(m),
                Msg::AccountLoaded(saved) => {
                    let a = &mut self.account;
                    a.loaded = true;
                    a.expired = false;
                    a.name = None;
                    a.checking = false;
                    a.saved = saved.clone();
                    if let Some(name) = a.just_logged.take().filter(|_| saved.is_some()) {
                        a.name = Some(name);
                    } else if let Some(s) = saved.filter(|s| s.has_session) {
                        a.checking = true;
                        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
                        std::thread::spawn(move || {
                            Self::send(&tx, &ctx, Msg::AccountVerified(connect::verify(&s)));
                        });
                    }
                }
                Msg::AccountVerified(r) => {
                    self.account.checking = false;
                    match r {
                        Ok(name) => self.account.name = Some(name),
                        Err(e) => {
                            self.log.push(format!("Iurefficient: {e}"));
                            self.account.expired = true;
                        }
                    }
                }
                Msg::LoginDone(r) => self.login_done(r),
                Msg::LogoutDone(r) => {
                    self.account.busy = false;
                    match r {
                        Ok(()) => self
                            .log
                            .push("Iurefficient: sesión cerrada en este equipo".into()),
                        Err(e) => self.account.error = Some(e),
                    }
                    self.load_account();
                }
                Msg::Done(i, result) => {
                    let row = &mut self.rows[i];
                    row.busy = None;
                    row.progress = None;
                    match result {
                        Ok(Some(m)) => {
                            self.log
                                .push(format!("{} {} instalada", APPS[i].name, m.version));
                            self.state.installs.insert(APPS[i].id.to_string(), m);
                            self.state.save();
                        }
                        Ok(None) => {
                            self.log.push(format!("{} desinstalada", APPS[i].name));
                            self.state.installs.remove(APPS[i].id);
                            self.state.save();
                        }
                        Err(e) => {
                            self.log.push(format!("{}: {e}", APPS[i].name));
                            row.error = Some(e);
                        }
                    }
                    self.redetect(i);
                }
            }
        }
        if self.log.len() > 300 {
            self.log.drain(..self.log.len() - 300);
        }
    }

    fn start_install(&mut self, i: usize) {
        let Some(sys) = self.sys.clone() else { return };
        let app: &'static AppDef = &APPS[i];
        let row = &self.rows[i];
        let Some(Ok(release)) = row.release.clone() else {
            return;
        };
        let pref = self.state.prefs.get(app.id);
        let variant = plan::variant(app, &sys, row.installed.as_ref(), pref);
        let Some(format) = plan::format(app, &sys, &release, variant, row.installed.as_ref(), pref)
        else {
            self.rows[i].error = Some(format!(
                "No hay instalador {} para este equipo",
                variant.label()
            ));
            return;
        };
        let previous = self.state.installs.get(app.id).cloned();
        let (tx, ctx, token) = (self.tx.clone(), self.ctx.clone(), self.token());
        let row = &mut self.rows[i];
        row.busy = Some(format!(
            "Instalando {} {}…",
            release.version(),
            variant.label()
        ));
        row.error = None;
        row.progress = Some(0.0);
        std::thread::spawn(move || {
            let (tx2, ctx2) = (tx.clone(), ctx.clone());
            let log =
                move |m: String| Self::send(&tx2, &ctx2, Msg::Log(format!("{}: {m}", app.name)));
            let (tx3, ctx3) = (tx.clone(), ctx.clone());
            let last = std::cell::Cell::new(0.0f32);
            let progress = move |p: f32| {
                if p - last.get() >= 0.005 || p >= 1.0 {
                    last.set(p);
                    Self::send(&tx3, &ctx3, Msg::Progress(i, p));
                }
            };
            let r = installer::install(
                app,
                &release,
                &sys,
                variant,
                format,
                token.as_deref(),
                previous.as_ref(),
                &log,
                &progress,
            );
            Self::send(&tx, &ctx, Msg::Done(i, r.map(Some)));
        });
    }

    fn start_uninstall(&mut self, i: usize) {
        let Some(inst) = self.rows[i].installed.clone() else {
            return;
        };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        self.rows[i].busy = Some("Desinstalando…".into());
        self.confirm_uninstall = None;
        std::thread::spawn(move || {
            let r = installer::uninstall(&APPS[i], &inst);
            Self::send(&tx, &ctx, Msg::Done(i, r.map(|_| None)));
        });
    }

    fn launch(&mut self, i: usize) {
        if let (Some(inst), Some(sys)) = (&self.rows[i].installed, &self.sys) {
            match installer::launch(inst, sys) {
                Ok(()) => self.log.push(format!("Abriendo {}", APPS[i].name)),
                Err(e) => self.rows[i].error = Some(e),
            }
        }
    }

    fn load_account(&self) {
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || Self::send(&tx, &ctx, Msg::AccountLoaded(connect::saved())));
    }

    fn open_login(&mut self) {
        let mut f = LoginForm::default();
        if let Some(s) = &self.account.saved {
            f.domain = s.domain.clone();
            f.email = s.email.clone();
        }
        self.login = Some(f);
    }

    fn submit_login(&mut self) {
        let Some(f) = self.login.as_mut() else { return };
        f.busy = true;
        f.error = None;
        let (domain, email, password) = (f.domain.clone(), f.email.clone(), f.password.clone());
        let totp = f.totp_token.clone().map(|t| (t, f.code.clone()));
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let totp = totp.as_ref().map(|(t, c)| (t.as_str(), c.as_str()));
            let r = connect::login(&domain, &email, &password, totp);
            Self::send(&tx, &ctx, Msg::LoginDone(r));
        });
    }

    fn login_done(&mut self, r: Result<connect::LoginOutcome, String>) {
        let Some(f) = self.login.as_mut() else { return };
        f.busy = false;
        match r {
            Ok(connect::LoginOutcome::TotpRequired { totp_token }) => {
                f.totp_token = Some(totp_token);
                f.code.clear();
            }
            Ok(connect::LoginOutcome::Ok {
                name,
                created_webdav,
            }) => {
                self.log
                    .push(format!("Iurefficient: conectada como {name}"));
                if created_webdav {
                    self.log.push(
                        "Iurefficient: se creó la contraseña de aplicación WebDAV de este equipo"
                            .into(),
                    );
                }
                self.login = None;
                self.account.error = None;
                self.account.just_logged = Some(name);
                self.load_account();
            }
            Err(e) => {
                // Un código TOTP erróneo obliga a empezar de nuevo.
                f.totp_token = None;
                f.error = Some(e);
            }
        }
    }

    fn logout(&mut self) {
        let Some(saved) = self.account.saved.clone() else {
            return;
        };
        self.account.busy = true;
        self.confirm_logout = false;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || Self::send(&tx, &ctx, Msg::LogoutDone(connect::logout(&saved))));
    }

    fn status(&self, i: usize) -> Status {
        match &self.sys {
            Some(sys) if self.rows[i].detected => plan::status(
                &APPS[i],
                sys,
                self.rows[i].release.as_ref(),
                self.rows[i].installed.as_ref(),
            ),
            _ => Status::Checking,
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Ajustes").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label("Token de GitHub (sólo para repos privados como IureVoz o IureProject):");
            ui.add(egui::TextEdit::singleline(&mut self.token_input).password(true).desired_width(320.0));
            ui.label(RichText::new("Basta un token de sólo lectura (Contents: read). Se guarda en el archivo de estado.").small());
            if ui.button("Guardar y volver a consultar").clicked() {
                let t = self.token_input.trim().to_string();
                self.state.github_token = (!t.is_empty()).then_some(t);
                self.state.save();
                self.refresh_releases();
            }
            ui.separator();
            ui.label(format!("Datos del lanzador: {}", crate::util::data_dir().display()));
            if ui.button("Restablecer variantes y formatos elegidos").clicked() {
                self.state.prefs.clear();
                self.state.save();
            }
        });
        self.show_settings = open;
    }
}
