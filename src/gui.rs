//! Interfaz gráfica (egui): una tarjeta por app con su estado y acciones.

use crate::catalog::{AppDef, Format, APPS};
use crate::github::{self, Release};
use crate::installer::{self, Installed};
use crate::plan::{self, Status};
use crate::state::{version_cmp, Managed, State};
use crate::system::{Os, SystemInfo};
use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc::{channel, Receiver, Sender};

const SELF_REPO: &str = "ellaguno/iureapps";

enum Msg {
    System(Box<SystemInfo>),
    Detected(usize, Option<Installed>),
    Release(usize, Result<Release, String>),
    SelfRelease(Release),
    Progress(usize, f32),
    Log(String),
    Done(usize, Result<Option<Managed>, String>),
}

#[derive(Default)]
struct Row {
    release: Option<Result<Release, String>>,
    installed: Option<Installed>,
    detected: bool,
    busy: Option<String>,
    progress: Option<f32>,
    error: Option<String>,
    confirm_uninstall: bool,
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
    token_input: String,
}

pub fn run() -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Iurefficient Apps")
            .with_inner_size([760.0, 720.0])
            .with_min_inner_size([520.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Iurefficient Apps",
        options,
        Box::new(|cc| Ok(Box::new(LauncherApp::new(cc.egui_ctx.clone())))),
    )
    .map_err(|e| e.to_string())
}

impl LauncherApp {
    fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = channel();
        let state = State::load();
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
        };
        app.spawn_system();
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
                }
                Msg::SelfRelease(r) => {
                    if version_cmp(r.version(), env!("CARGO_PKG_VERSION")).is_gt() {
                        self.self_update = Some(r);
                    }
                }
                Msg::Progress(i, p) => self.rows[i].progress = Some(p),
                Msg::Log(m) => self.log.push(m),
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
        self.rows[i].confirm_uninstall = false;
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

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Iurefficient Apps").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Ajustes").clicked() {
                    self.show_settings = !self.show_settings;
                }
                if ui.button("Buscar actualizaciones").clicked() {
                    self.refresh_releases();
                    for i in 0..APPS.len() {
                        self.redetect(i);
                    }
                }
                let pending: Vec<usize> = (0..APPS.len())
                    .filter(|&i| {
                        self.status(i) == Status::UpdateAvailable && self.rows[i].busy.is_none()
                    })
                    .collect();
                if !pending.is_empty()
                    && ui
                        .button(format!("Actualizar todo ({})", pending.len()))
                        .clicked()
                {
                    for i in pending {
                        self.start_install(i);
                    }
                }
            });
        });
        if let Some(r) = &self.self_update {
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(230, 160, 40),
                    format!("Hay una versión nueva del lanzador: {}", r.version()),
                );
                ui.hyperlink_to("Descargar", &r.html_url);
            });
        }
    }

    fn system_panel(&self, ui: &mut egui::Ui) {
        let Some(sys) = &self.sys else {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Analizando el equipo…");
            });
            return;
        };
        egui::CollapsingHeader::new(
            RichText::new(format!("Este equipo: {}", sys.platform_label())).strong(),
        )
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("sys")
                .num_columns(2)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label("CPU");
                    let avx = match sys.avx2 {
                        Some(true) => " · AVX2",
                        Some(false) => " · sin AVX2 (Whisper irá lento)",
                        None => "",
                    };
                    ui.label(format!("{} · {} hilos{avx}", sys.cpu, sys.cores));
                    ui.end_row();
                    if let Some(r) = sys.ram_gb {
                        ui.label("Memoria");
                        ui.label(format!("{r:.1} GB"));
                        ui.end_row();
                    }
                    ui.label("GPU");
                    ui.label(if sys.gpus.is_empty() {
                        "no detectada".to_string()
                    } else {
                        sys.gpus
                            .iter()
                            .map(|g| g.name.as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    });
                    ui.end_row();
                    ui.label("CUDA");
                    ui.label(match &sys.nvidia {
                        Some(n) => format!(
                            "driver NVIDIA {}{}",
                            n.driver,
                            n.vram_mb
                                .map(|m| format!(" · {:.1} GB VRAM", m as f64 / 1024.0))
                                .unwrap_or_default()
                        ),
                        None => "no".into(),
                    });
                    ui.end_row();
                    ui.label("Vulkan");
                    ui.label(if sys.vulkan { "sí" } else { "no" });
                    ui.end_row();
                    if sys.os == Os::Linux {
                        ui.label("Paquetes");
                        ui.label(format!(
                            "{:?} · AppImage {} · {} {}",
                            sys.pkg,
                            if sys.fuse {
                                "con FUSE"
                            } else {
                                "sin FUSE (se extrae al abrir)"
                            },
                            sys.desktop,
                            sys.session
                        ));
                        ui.end_row();
                    }
                });
        });
    }

    fn app_card(&mut self, ui: &mut egui::Ui, i: usize) {
        let app: &'static AppDef = &APPS[i];
        let status = self.status(i);
        let Some(sys) = self.sys.clone() else { return };
        let (badge_color, badge) = match status {
            Status::UpToDate | Status::InstalledUnknown => {
                (Color32::from_rgb(60, 170, 90), status.label())
            }
            Status::UpdateAvailable => (Color32::from_rgb(230, 150, 30), status.label()),
            Status::NotInstalled => (Color32::from_rgb(90, 140, 220), status.label()),
            _ => (Color32::GRAY, status.label()),
        };

        egui::Frame::group(ui.style())
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(app.name).size(18.0).strong());
                    ui.label(RichText::new(badge).color(badge_color).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let latest = match &self.rows[i].release {
                            Some(Ok(r)) => format!("última {} ({})", r.version(), r.date()),
                            Some(Err(_)) => String::new(),
                            None => "…".into(),
                        };
                        let inst = self.rows[i]
                            .installed
                            .as_ref()
                            .map(|x| {
                                format!(
                                    "instalada {} · {}{} · ",
                                    x.version.as_deref().unwrap_or("?"),
                                    x.format.label(),
                                    x.variant
                                        .filter(|_| app.gpu_variants)
                                        .map(|v| format!(" {}", v.label()))
                                        .unwrap_or_default()
                                )
                            })
                            .unwrap_or_default();
                        ui.label(RichText::new(format!("{inst}{latest}")).weak());
                    });
                });
                ui.label(RichText::new(app.description).weak());
                if !app.note.is_empty() {
                    ui.label(RichText::new(app.note).small().italics());
                }
                if let Some(Err(e)) = &self.rows[i].release {
                    if self.rows[i].installed.is_none() || status == Status::InstalledUnknown {
                        ui.label(RichText::new(e).small().color(Color32::GRAY));
                    }
                }

                let release = match &self.rows[i].release {
                    Some(Ok(r)) => Some(r.clone()),
                    _ => None,
                };
                let installed = self.rows[i].installed.clone();

                // Selector de aceleración y formato.
                if let Some(release) = &release {
                    if app.supports(&sys) {
                        let pref = self.state.prefs.get(app.id).cloned();
                        let variant = plan::variant(app, &sys, installed.as_ref(), pref.as_ref());
                        let (recommended, why) = app.recommend_variant(&sys);
                        let mut new_variant = variant;
                        let mut new_format = None;
                        let formats_here = app
                            .formats(&sys)
                            .into_iter()
                            .filter(|f| app.select_asset(release, &sys, variant, *f).is_some())
                            .count();
                        if app.gpu_variants || formats_here > 1 {
                            ui.horizontal(|ui| {
                                if app.gpu_variants {
                                    ui.label("Aceleración:");
                                    egui::ComboBox::from_id_salt(("variant", i))
                                        .selected_text(variant.label())
                                        .show_ui(ui, |ui| {
                                            for v in app.variants(&sys) {
                                                let ok =
                                                    app.best_format(release, &sys, v).is_some();
                                                let mut text = v.label().to_string();
                                                if v == recommended {
                                                    text.push_str("  (recomendada)");
                                                }
                                                if !ok {
                                                    text.push_str("  (no disponible)");
                                                }
                                                ui.add_enabled_ui(ok, |ui| {
                                                    ui.selectable_value(&mut new_variant, v, text)
                                                });
                                            }
                                        });
                                }
                                let formats: Vec<Format> = app
                                    .formats(&sys)
                                    .into_iter()
                                    .filter(|f| {
                                        app.select_asset(release, &sys, variant, *f).is_some()
                                    })
                                    .collect();
                                if let Some(current) = plan::format(
                                    app,
                                    &sys,
                                    release,
                                    variant,
                                    installed.as_ref(),
                                    pref.as_ref(),
                                ) {
                                    if formats.len() > 1 {
                                        ui.label("Formato:");
                                        let mut f = current;
                                        egui::ComboBox::from_id_salt(("format", i))
                                            .selected_text(f.label())
                                            .show_ui(ui, |ui| {
                                                for x in &formats {
                                                    ui.selectable_value(&mut f, *x, x.label());
                                                }
                                            });
                                        if f != current {
                                            new_format = Some(f);
                                        }
                                    } else if app.gpu_variants {
                                        ui.label(RichText::new(current.label()).weak());
                                    }
                                }
                            });
                        }
                        if app.gpu_variants && !why.is_empty() {
                            ui.label(RichText::new(format!("Recomendada: {why}")).small());
                        }
                        if new_variant != variant || new_format.is_some() {
                            let p = self.state.prefs.entry(app.id.to_string()).or_default();
                            if new_variant != variant {
                                p.variant = Some(new_variant);
                                p.format = None;
                            }
                            if let Some(f) = new_format {
                                p.format = Some(f);
                            }
                            self.state.save();
                        }
                    }
                }

                // Acciones.
                ui.horizontal(|ui| {
                    let busy = self.rows[i].busy.is_some();
                    if let Some(text) = &self.rows[i].busy {
                        ui.spinner();
                        ui.label(text);
                        if let Some(p) = self.rows[i].progress {
                            ui.add(
                                egui::ProgressBar::new(p)
                                    .desired_width(200.0)
                                    .show_percentage(),
                            );
                        }
                        return;
                    }
                    let can_install = release.is_some() && app.supports(&sys);
                    let action = match status {
                        Status::NotInstalled => Some("Instalar"),
                        Status::UpdateAvailable => Some("Actualizar"),
                        _ => None,
                    };
                    if let Some(label) = action.filter(|_| can_install) {
                        if ui.button(RichText::new(label).strong()).clicked() {
                            self.start_install(i);
                        }
                    }
                    if let (Some(inst), Some(release)) = (&installed, &release) {
                        // Cambio de variante o de formato: reinstalar.
                        let pref = self.state.prefs.get(app.id);
                        let v = plan::variant(app, &sys, Some(inst), pref);
                        let f = plan::format(app, &sys, release, v, Some(inst), pref);
                        let differs = (app.gpu_variants && inst.variant.is_some_and(|iv| iv != v))
                            || f.is_some_and(|f| f != inst.format);
                        if status == Status::UpToDate
                            && differs
                            && ui.button("Cambiar a la selección").clicked()
                        {
                            self.start_install(i);
                        }
                    }
                    if installed.is_some() {
                        if ui.button("Abrir").clicked() {
                            self.launch(i);
                        }
                        if self.rows[i].confirm_uninstall {
                            if ui
                                .button(RichText::new("¿Seguro? Desinstalar").color(Color32::RED))
                                .clicked()
                            {
                                self.start_uninstall(i);
                            }
                            if ui.button("Cancelar").clicked() {
                                self.rows[i].confirm_uninstall = false;
                            }
                        } else if ui
                            .add_enabled(!busy, egui::Button::new("Desinstalar"))
                            .clicked()
                        {
                            self.rows[i].confirm_uninstall = true;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let url = release
                            .as_ref()
                            .map(|r| r.html_url.clone())
                            .unwrap_or_else(|| app.release_url());
                        let notes = release
                            .as_ref()
                            .and_then(|r| r.body.clone())
                            .unwrap_or_default();
                        let link = ui.hyperlink_to("Novedades", url);
                        if !notes.trim().is_empty() {
                            let short: String = notes.chars().take(900).collect();
                            link.on_hover_text(short);
                        }
                    });
                });
                if let Some(e) = &self.rows[i].error {
                    ui.label(RichText::new(e).color(Color32::from_rgb(220, 70, 70)));
                }
            });
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

impl eframe::App for LauncherApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_messages();
        let ctx = ui.ctx().clone();

        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.top_bar(ui);
            ui.add_space(4.0);
        });
        egui::Panel::bottom("log")
            .resizable(true)
            .default_size(90.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.log.is_empty() {
                            ui.label(RichText::new("Registro de actividad").weak());
                        }
                        for l in &self.log {
                            ui.label(RichText::new(l).small().monospace());
                        }
                    });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.system_panel(ui);
            ui.add_space(6.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for i in 0..APPS.len() {
                    self.app_card(ui, i);
                    ui.add_space(6.0);
                }
            });
        });
        if self.show_settings {
            self.settings_window(&ctx);
        }
    }
}
