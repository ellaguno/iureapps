//! Interfaz gráfica (egui): mosaico de apps con su icono, estado y acciones.

mod detail;
mod extras;
mod markdown;
mod system_theme;
mod theme;
mod view;

use crate::catalog::{AppDef, APPS, EXTRAS};
use crate::connect;
use crate::github::{self, Release};
use crate::installer::{self, Installed};
use crate::plan::{self, Status};
use crate::state::{version_cmp, Managed, State, ThemePref};
use crate::system::SystemInfo;
use crate::{instance, selfupdate};
use eframe::egui::{self, RichText};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

enum Msg {
    System(Box<SystemInfo>),
    Detected(usize, Option<Installed>),
    Release(usize, Result<Release, String>),
    /// Última release de una de las apps de la carpeta (`EXTRAS`), entre ellas el lanzador.
    ExtraRelease(usize, Result<Release, String>),
    Progress(usize, f32),
    Log(String),
    Done(usize, Result<Option<Managed>, String>),
    AccountLoaded(Option<connect::Saved>),
    AccountVerified(Result<String, String>),
    LoginDone(Result<connect::LoginOutcome, String>),
    LogoutDone(Result<(), String>),
    /// Capturas de la app con ese id (de `APPS` o de `EXTRAS`).
    Shots(
        &'static str,
        Result<Vec<(String, egui::ColorImage)>, String>,
    ),
    /// Otra instancia pidió que esta ventana se muestre.
    Wake,
    SelfProgress(f32),
    SelfDone(Result<selfupdate::Relaunch, String>),
}

/// Actualización del propio lanzador.
enum SelfUpdate {
    Running(f32),
    Failed(String),
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

/// Ficha abierta en lugar del mosaico.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Selected {
    Account,
    App(usize),
    /// La carpeta «Más apps», con las apps de `EXTRAS`.
    Folder,
    Extra(usize),
}

/// Capturas de la ficha de una app.
enum Shots {
    Loading,
    /// Pie de foto y textura; vacía si la app no publica capturas.
    Ready(Vec<(String, egui::TextureHandle)>),
    Failed(String),
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
    selected: Option<Selected>,
    /// Capturas por id de app.
    shots: HashMap<&'static str, Shots>,
    /// Captura ampliada: (id de la app, índice).
    viewer: Option<(&'static str, usize)>,
    /// Releases de las apps de la carpeta, en el orden de `EXTRAS`.
    extras: Vec<Option<Result<Release, String>>>,
    /// Qué se copió al portapapeles y cuándo (para confirmarlo en el botón).
    copied: Option<(String, Instant)>,
    self_updating: Option<SelfUpdate>,
}

pub fn run() -> Result<(), String> {
    // Ya abierto: se trae al frente esa ventana. Tras actualizarse, la versión nueva no cede
    // ante la anterior, que se está cerrando.
    if std::env::var_os(selfupdate::RESTARTED_ENV).is_some() {
        std::env::remove_var(selfupdate::RESTARTED_ENV);
    } else if instance::wake_existing() {
        return Ok(());
    }
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

/// Errores al abrir enlaces, que se pasan al registro en el siguiente fotograma.
static LINK_ERRORS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Abre `url` en el navegador sin bloquear la interfaz.
fn open_link(url: impl AsRef<str>) {
    let url = url.as_ref().to_string();
    std::thread::spawn(move || {
        if let Err(e) = crate::util::open_url(&url) {
            if let Ok(mut errors) = LINK_ERRORS.lock() {
                errors.push(e);
            }
        }
    });
}

/// Nombre de la app con ese id, de la suite o de la carpeta.
fn app_name(id: &str) -> &'static str {
    APPS.iter()
        .map(|a| (a.id, a.name))
        .chain(EXTRAS.iter().map(|e| (e.id, e.name)))
        .find(|(i, _)| *i == id)
        .map_or("", |(_, n)| n)
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
            selected: None,
            shots: HashMap::new(),
            viewer: None,
            extras: EXTRAS.iter().map(|_| None).collect(),
            copied: None,
            self_updating: None,
        };
        let (tx, ctx2) = (app.tx.clone(), app.ctx.clone());
        instance::listen(move || Self::send(&tx, &ctx2, Msg::Wake));
        system_theme::start(&app.ctx);
        app.sync_theme();
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
        // Las de la carpeta, entre ellas el propio lanzador (aviso de versión nueva).
        for (k, extra) in EXTRAS.iter().enumerate() {
            self.extras[k] = None;
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let r = github::latest_release(extra.repo, None);
                Self::send(&tx, &ctx, Msg::ExtraRelease(k, r));
            });
        }
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
                Msg::ExtraRelease(k, r) => {
                    match &r {
                        // La versión de Microsoft Store la actualiza la Store: no se avisa.
                        Ok(rel)
                            if EXTRAS[k].repo == selfupdate::REPO
                                && !selfupdate::from_store()
                                && version_cmp(rel.version(), env!("CARGO_PKG_VERSION"))
                                    .is_gt() =>
                        {
                            self.self_update = Some(rel.clone());
                        }
                        Ok(_) => {}
                        Err(e) => self.log.push(format!("{}: {e}", EXTRAS[k].name)),
                    }
                    self.extras[k] = Some(r);
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
                Msg::Wake => {
                    use egui::ViewportCommand as V;
                    self.ctx.send_viewport_cmd(V::Visible(true));
                    self.ctx.send_viewport_cmd(V::Minimized(false));
                    self.ctx.send_viewport_cmd(V::Focus);
                    self.ctx.send_viewport_cmd(V::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ));
                }
                Msg::SelfProgress(p) => self.self_updating = Some(SelfUpdate::Running(p)),
                Msg::SelfDone(r) => self.self_update_done(r),
                Msg::Shots(id, r) => {
                    let shots = match r {
                        Ok(images) => Shots::Ready(
                            images
                                .into_iter()
                                .enumerate()
                                .map(|(k, (caption, img))| {
                                    let name = format!("shot-{id}-{k}");
                                    let tex = self.ctx.load_texture(
                                        name,
                                        img,
                                        egui::TextureOptions::LINEAR,
                                    );
                                    (caption, tex)
                                })
                                .collect(),
                        ),
                        Err(e) => {
                            self.log.push(format!("{}: capturas: {e}", app_name(id)));
                            Shots::Failed(e)
                        }
                    };
                    self.shots.insert(id, shots);
                }
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
        if let Ok(mut errors) = LINK_ERRORS.lock() {
            self.log.append(&mut errors);
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

    fn start_self_update(&mut self) {
        let (Some(sys), Some(release)) = (&self.sys, &self.self_update) else {
            return;
        };
        let plan = match selfupdate::plan(release, sys) {
            Ok(p) => p,
            Err(e) => {
                self.self_updating = Some(SelfUpdate::Failed(e));
                return;
            }
        };
        self.log.push(format!(
            "Iurefficient Apps: actualizando a {}",
            release.version()
        ));
        self.self_updating = Some(SelfUpdate::Running(0.0));
        let (tx, ctx, token) = (self.tx.clone(), self.ctx.clone(), self.token());
        std::thread::spawn(move || {
            let (tx2, ctx2) = (tx.clone(), ctx.clone());
            let log = move |m: String| {
                Self::send(&tx2, &ctx2, Msg::Log(format!("Iurefficient Apps: {m}")))
            };
            let (tx3, ctx3) = (tx.clone(), ctx.clone());
            let last = std::cell::Cell::new(0.0f32);
            let progress = move |p: f32| {
                if p - last.get() >= 0.01 || p >= 1.0 {
                    last.set(p);
                    Self::send(&tx3, &ctx3, Msg::SelfProgress(p));
                }
            };
            let r = selfupdate::apply(&plan, token.as_deref(), &log, &progress);
            Self::send(&tx, &ctx, Msg::SelfDone(r));
        });
    }

    /// Instalada la versión nueva: se abre y esta ventana se cierra.
    fn self_update_done(&mut self, r: Result<selfupdate::Relaunch, String>) {
        let r = r.and_then(|target| {
            instance::release();
            selfupdate::relaunch(&target)
        });
        match r {
            Ok(()) => {
                self.log
                    .push("Iurefficient Apps: versión nueva instalada; reiniciando".into());
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => {
                self.log.push(format!("Iurefficient Apps: {e}"));
                self.self_updating = Some(SelfUpdate::Failed(e));
            }
        }
    }

    /// Descarga (o lee de la caché) las capturas de la app `id` (repo `repo`), una vez por
    /// sesión.
    fn load_shots(&mut self, id: &'static str, repo: &'static str) {
        if self.shots.contains_key(id) {
            return;
        }
        self.shots.insert(id, Shots::Loading);
        let (tx, ctx, token) = (self.tx.clone(), self.ctx.clone(), self.token());
        std::thread::spawn(move || {
            let r = crate::shots::load(id, repo, token.as_deref()).map(|shots| {
                shots
                    .into_iter()
                    .filter_map(|s| Some((s.caption, theme::decode_shot(&s.png)?)))
                    .collect()
            });
            Self::send(&tx, &ctx, Msg::Shots(id, r));
        });
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

    /// Tema claro u oscuro según la preferencia (o el sistema); se llama en cada fotograma.
    fn sync_theme(&self) {
        let dark = match self.state.theme {
            ThemePref::Dark => true,
            ThemePref::Light => false,
            // Sin saber el del sistema, el oscuro de siempre.
            ThemePref::System => system_theme::is_dark(&self.ctx).unwrap_or(true),
        };
        if dark != theme::is_dark() || self.ctx.theme() != Self::egui_theme(dark) {
            theme::apply(&self.ctx, dark);
        }
    }

    fn egui_theme(dark: bool) -> egui::Theme {
        if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new("Ajustes").open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label("Apariencia:");
            ui.horizontal(|ui| {
                let mut pref = self.state.theme;
                ui.selectable_value(&mut pref, ThemePref::System, "Como el sistema");
                ui.selectable_value(&mut pref, ThemePref::Light, "Clara");
                ui.selectable_value(&mut pref, ThemePref::Dark, "Oscura");
                if pref != self.state.theme {
                    self.state.theme = pref;
                    self.state.save();
                    self.sync_theme();
                }
            });
            if self.state.theme == ThemePref::System {
                let now = match system_theme::is_dark(ctx) {
                    Some(true) => "El sistema usa el tema oscuro.",
                    Some(false) => "El sistema usa el tema claro.",
                    None => "No se pudo saber el tema del sistema: se usa el oscuro.",
                };
                ui.label(RichText::new(now).small());
            }
            ui.separator();
            ui.label("Token de GitHub (sólo para repos privados):");
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
