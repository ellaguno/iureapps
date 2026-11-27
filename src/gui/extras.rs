//! Carpeta «Más apps» del mosaico (otras apps de Iurefficient que el lanzador no instala,
//! con su ficha y sus formas de instalarlas) y las apps móviles de Iurefficient.

use super::detail::{button, fact, link, section, Kind};
use super::theme::{self, pal};
use super::view::{caps, caps_label, nice_date, two_tone, Badge, Cell, CELL_W, FOLDER};
use super::{Filter, LauncherApp, Selected};
use crate::catalog::{ExtraDef, WayAction, EXTRAS, MOBILE_ANDROID, MOBILE_IOS};
use crate::github::Release;
use crate::system::SystemInfo;
use eframe::egui::{self, vec2, Color32, CornerRadius, Painter, Rect, RichText, Stroke, Ui};
use std::time::{Duration, Instant};

/// ¿Es el propio lanzador?
fn is_self(e: &ExtraDef) -> bool {
    e.repo == crate::selfupdate::REPO
}

/// Etiqueta corta para un botón: «Descargar .deb (Debian, Ubuntu)» → «Descargar .deb»,
/// «Copiar comando de instalación» → «Copiar comando».
fn short(label: &str) -> String {
    let label = label.split(" (").next().unwrap_or_default();
    match label.split_once(" de ") {
        Some((head, _)) if label.chars().count() > 22 => head.to_string(),
        _ => label.to_string(),
    }
}

impl LauncherApp {
    /// La carpeta se ve sin filtro; con búsqueda, si alguna de sus apps coincide.
    pub(super) fn folder_matches(&self) -> bool {
        let q = self.search.trim().to_lowercase();
        self.filter == Filter::All
            && (q.is_empty()
                || ["más apps", "carpeta"].iter().any(|t| t.contains(&q))
                || (0..EXTRAS.len()).any(|k| self.extra_matches(k)))
    }

    fn extra_matches(&self, k: usize) -> bool {
        let e = &EXTRAS[k];
        let q = self.search.trim().to_lowercase();
        q.is_empty()
            || [e.name, e.tagline, e.hint, e.description, e.platforms]
                .iter()
                .any(|t| t.to_lowercase().contains(&q))
    }

    fn extra_release(&self, k: usize) -> Option<&Release> {
        self.extras[k].as_ref().and_then(|r| r.as_ref().ok())
    }

    /// Estado de una app de la carpeta: (color, texto).
    fn extra_status(&self, k: usize) -> (Color32, String) {
        let e = &EXTRAS[k];
        match &self.self_update {
            Some(r) if is_self(e) => (pal().warn, format!("Actualización {}", r.version())),
            _ if is_self(e) => (pal().ok, format!("{} · en uso", e.kind)),
            _ => (pal().info, e.kind.to_string()),
        }
    }

    /// Icono de la carpeta: un cuadro con los iconos de su contenido en 2×2, como en el
    /// teléfono.
    pub(super) fn paint_folder(&self, painter: &Painter, rect: Rect) {
        let s = rect.width();
        painter.rect(
            rect,
            CornerRadius::same((s * 0.22) as u8),
            pal().surface_hi,
            Stroke::new(1.0, pal().line),
            egui::StrokeKind::Inside,
        );
        let gap = s * 0.1;
        let mini = (s - 3.0 * gap) / 2.0;
        for (k, e) in EXTRAS.iter().take(4).enumerate() {
            let at =
                rect.min + vec2(gap, gap) + vec2((k % 2) as f32, (k / 2) as f32) * (mini + gap);
            let r = Rect::from_min_size(at, vec2(mini, mini));
            self.icons.paint(painter, r, e.id, theme::accent(e.accent));
        }
    }

    pub(super) fn folder_cell(&mut self, ui: &mut Ui, w: f32) {
        let n = EXTRAS.len();
        let cell = Cell {
            icon_id: FOLDER,
            accent: pal().info,
            name: two_tone("Más apps", 14.5, pal().info),
            line: (
                format!("{n} {}", if n == 1 { "app" } else { "apps" }),
                pal().muted,
            ),
            // Avisa si el propio lanzador tiene versión nueva.
            badge: self.self_update.as_ref().map(|_| Badge::Update),
            dim: false,
            progress: None,
        };
        let names: Vec<&str> = EXTRAS.iter().map(|e| e.name).collect();
        let resp = self
            .paint_cell(ui, w, cell)
            .on_hover_text(format!("Otras apps de Iurefficient: {}", names.join(", ")));
        if resp.clicked() {
            self.selected = Some(Selected::Folder);
        }
    }

    /// Contenido de la carpeta: sus apps en mosaico; cada una abre su ficha.
    pub(super) fn folder_view(&mut self, ui: &mut Ui, sys: &SystemInfo) {
        ui.label(two_tone("Más apps", 30.0, pal().info));
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Otras apps de Iurefficient. No las instala el lanzador: cada ficha dice cómo \
                 instalarlas.",
            )
            .size(14.0)
            .color(pal().muted),
        );
        ui.add_space(18.0);
        let items: Vec<usize> = (0..EXTRAS.len())
            .filter(|&k| self.extra_matches(k))
            .collect();
        let width = ui.available_width();
        let cols = ((width / CELL_W).floor() as usize).max(1);
        let cell_w = (width / cols as f32).floor().min(CELL_W * 1.3);
        for row in items.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for &k in row {
                    self.extra_cell(ui, k, sys, cell_w);
                }
            });
        }
    }

    fn extra_cell(&mut self, ui: &mut Ui, k: usize, sys: &SystemInfo, w: f32) {
        let e: &'static ExtraDef = &EXTRAS[k];
        let accent = theme::accent(e.accent);
        let (color, text) = self.extra_status(k);
        let badge = match &self.self_update {
            Some(_) if is_self(e) => Some(Badge::Update),
            None if is_self(e) => Some(Badge::Ok),
            _ => None,
        };
        let line = if is_self(e) && self.self_update.is_some() {
            (text.clone(), color)
        } else {
            (e.tagline.to_string(), pal().muted)
        };
        let cell = Cell {
            icon_id: e.id,
            accent,
            name: two_tone(e.name, 14.5, accent),
            line,
            badge,
            dim: false,
            progress: None,
        };
        let resp = self
            .paint_cell(ui, w, cell)
            .on_hover_text(format!("{} · {text}", e.tagline));
        if resp.clicked() {
            self.selected = Some(Selected::Extra(k));
        }
        resp.context_menu(|ui| {
            ui.set_min_width(240.0);
            if ui.button("Ver detalles").clicked() {
                self.selected = Some(Selected::Extra(k));
            }
            ui.separator();
            self.ways_menu(ui, k, sys);
        });
    }

    /// Las formas de instalar la app `k`, como botones de menú.
    fn ways_menu(&mut self, ui: &mut Ui, k: usize, _sys: &SystemInfo) {
        let e: &'static ExtraDef = &EXTRAS[k];
        for (w, way) in e.ways.iter().enumerate() {
            let label = if self.just_copied(k, w) {
                "Copiado ✓"
            } else {
                way.label
            };
            let mut b = ui.button(label);
            if let WayAction::Command(text) = way.action {
                b = b.on_hover_text(text);
            }
            if b.clicked() {
                self.run_way(ui.ctx(), k, w);
            }
        }
    }

    /// Hace lo que dice una forma de instalar: abre la descarga o la página, o copia el
    /// comando al portapapeles.
    fn run_way(&mut self, ctx: &egui::Context, k: usize, w: usize) {
        let e = &EXTRAS[k];
        match e.ways[w].action {
            WayAction::Asset(suffix) => {
                let url = self
                    .extra_release(k)
                    .and_then(|r| r.assets.iter().find(|a| a.name.ends_with(suffix)))
                    .map(|a| a.browser_download_url.clone())
                    .unwrap_or_else(|| e.release_url());
                super::open_link(url);
            }
            WayAction::Command(text) => {
                ctx.copy_text(text.to_string());
                self.log
                    .push(format!("{}: copiado al portapapeles: {text}", e.name));
                self.copied = Some((format!("{k}:{w}"), Instant::now()));
            }
            WayAction::Url(url) => super::open_link(url),
        }
    }

    /// «Copiado ✓» durante un par de segundos en lo que se acaba de copiar.
    fn just_copied(&self, k: usize, w: usize) -> bool {
        let fresh = self.copied.as_ref().is_some_and(|(key, at)| {
            *key == format!("{k}:{w}") && at.elapsed() < Duration::from_secs(2)
        });
        if fresh {
            self.ctx.request_repaint_after(Duration::from_millis(500));
        }
        fresh
    }

    /// Ficha de una app de la carpeta, como la de las de la suite: portada, acciones, formas
    /// de instalarla, capturas, detalles y novedades.
    pub(super) fn extra_detail(&mut self, ui: &mut Ui, k: usize, sys: &SystemInfo) {
        let e: &'static ExtraDef = &EXTRAS[k];
        let accent = theme::accent(e.accent);
        self.hero(ui, e.id, accent, |ui| {
            ui.add(egui::Label::new(two_tone(e.name, 30.0, accent)).truncate());
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{} · {}", e.tagline, e.hint))
                        .size(14.0)
                        .color(pal().muted),
                )
                .truncate(),
            );
        });

        self.extra_actions(ui, k, sys);

        section(ui, "Formas de instalarla");
        egui::Grid::new(("ways", k))
            .num_columns(2)
            .spacing([14.0, 10.0])
            .show(ui, |ui| {
                for (w, way) in e.ways.iter().enumerate() {
                    let verb = match way.action {
                        _ if self.just_copied(k, w) => "Copiado ✓",
                        WayAction::Asset(_) => "Descargar",
                        WayAction::Command(_) => "Copiar",
                        WayAction::Url(_) => "Abrir",
                    };
                    let recommended = e.primary_way(sys).is_some_and(|p| std::ptr::eq(p, way));
                    let kind = if recommended {
                        Kind::Install
                    } else {
                        Kind::Plain
                    };
                    if button(ui, verb, kind).clicked() {
                        self.run_way(ui.ctx(), k, w);
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let mut text = RichText::new(way.label).size(14.0).color(pal().text);
                        if recommended {
                            text = text.strong();
                        }
                        ui.label(text);
                        if let WayAction::Command(cmd) = way.action {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(cmd)
                                        .font(theme::mono(11.5))
                                        .color(pal().muted),
                                )
                                .wrap(),
                            );
                        }
                    });
                    ui.end_row();
                }
            });

        section(ui, "Capturas");
        self.shots_strip(ui, e.id, e.repo);

        section(ui, "Detalles");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(18.0, 14.0);
            let (color, text) = self.extra_status(k);
            fact(ui, "Estado", &text, color);
            if is_self(e) {
                fact(ui, "En uso", env!("CARGO_PKG_VERSION"), pal().text);
            }
            let latest = match &self.extras[k] {
                Some(Ok(r)) => format!("{} · {}", r.version(), nice_date(r.date())),
                Some(Err(err)) => err.clone(),
                None => "Consultando…".into(),
            };
            fact(ui, "Última versión", &latest, pal().text);
            fact(ui, "Plataformas", e.platforms, pal().text);
            fact(ui, "Repositorio", e.repo, pal().muted);
        });

        section(ui, "Acerca de");
        ui.label(RichText::new(e.description).size(15.0).color(pal().text));
        if let Some(r) = self.extra_release(k) {
            let notes = r.body.as_deref().unwrap_or("").trim().to_string();
            let version = r.version().to_string();
            if !notes.is_empty() {
                section(ui, &format!("Novedades de la versión {version}"));
                egui::Frame::new()
                    .fill(pal().surface)
                    .stroke(Stroke::new(1.0, pal().line))
                    .inner_margin(14)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        super::markdown::show(ui, &notes);
                    });
            }
        }
    }

    fn extra_actions(&mut self, ui: &mut Ui, k: usize, sys: &SystemInfo) {
        let e: &'static ExtraDef = &EXTRAS[k];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            if is_self(e) {
                if let Some(super::SelfUpdate::Running(p)) = self.self_updating {
                    ui.spinner();
                    caps_label(
                        ui,
                        &format!("Actualizando… {:.0}%", p * 100.0),
                        10.5,
                        pal().muted,
                    );
                    return;
                }
                if let Some(r) = self.self_update.clone() {
                    let can = crate::selfupdate::plan(&r, sys).is_ok()
                        && !matches!(self.self_updating, Some(super::SelfUpdate::Failed(_)));
                    if can {
                        if button(ui, "Actualizar y reiniciar", Kind::Update).clicked() {
                            self.start_self_update();
                        }
                    } else if button(ui, "Descargar la nueva", Kind::Update).clicked() {
                        super::open_link(&r.html_url);
                    }
                }
            }
            match e.primary_way(sys) {
                Some(p) => {
                    let w = e.ways.iter().position(|x| std::ptr::eq(x, p)).unwrap_or(0);
                    let label = if self.just_copied(k, w) {
                        "Copiado ✓".to_string()
                    } else {
                        short(p.label)
                    };
                    // Del lanzador, en uso, la descarga es para compartirla: botón discreto.
                    let kind = if is_self(e) {
                        Kind::Plain
                    } else {
                        Kind::Install
                    };
                    if button(ui, &label, kind).on_hover_text(p.label).clicked() {
                        self.run_way(ui.ctx(), k, w);
                    }
                }
                None => {
                    ui.label(
                        RichText::new(format!("No corre en {}.", sys.platform_label()))
                            .color(pal().warn),
                    );
                }
            }
            ui.add_space(10.0);
            let notes = self
                .extra_release(k)
                .map(|r| r.html_url.clone())
                .unwrap_or_else(|| e.release_url());
            link(ui, "Novedades ↗", &notes);
            ui.add_space(6.0);
            link(
                ui,
                "Versiones ↗",
                &format!("https://github.com/{}/releases", e.repo),
            );
            ui.add_space(6.0);
            link(ui, "GitHub ↗", &format!("https://github.com/{}", e.repo));
        });
        if let (true, Some(super::SelfUpdate::Failed(err))) = (is_self(e), &self.self_updating) {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("No se pudo actualizar: {err}"))
                    .size(13.0)
                    .color(pal().error),
            );
        }
    }

    /// Filas de las apps de la carpeta en la vista de tabla.
    pub(super) fn extra_rows(&mut self, ui: &mut Ui, sys: &SystemInfo) {
        if !self.folder_matches() {
            return;
        }
        let items: Vec<usize> = (0..EXTRAS.len())
            .filter(|&k| self.extra_matches(k))
            .collect();
        for k in items {
            let e: &'static ExtraDef = &EXTRAS[k];
            let accent = theme::accent(e.accent);
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
            self.icons.paint(ui.painter(), r, e.id, accent);
            ui.vertical(|ui| {
                ui.set_min_width(230.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let name = egui::Label::new(two_tone(e.name, 15.0, accent))
                    .extend()
                    .sense(egui::Sense::click());
                if ui
                    .add(name)
                    .on_hover_text("Ver detalles")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    self.selected = Some(Selected::Extra(k));
                }
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("{} · Más apps", e.tagline))
                            .size(12.0)
                            .color(pal().faint),
                    )
                    .extend(),
                );
            });
            let (c, t) = self.extra_status(k);
            ui.label(caps(&t, 10.0, c));
            let inst = if is_self(e) {
                env!("CARGO_PKG_VERSION").to_string()
            } else {
                "—".into()
            };
            ui.label(RichText::new(inst).color(pal().muted));
            let latest = match &self.extras[k] {
                Some(Ok(r)) => format!("{} · {}", r.version(), nice_date(r.date())),
                Some(Err(_)) => "—".into(),
                None => "…".into(),
            };
            ui.label(RichText::new(latest).color(pal().muted));
            ui.horizontal(|ui| {
                if super::view::flat_button(ui, "Detalles", Some(pal().text), vec2(146.0, 36.0))
                    .clicked()
                {
                    self.selected = Some(Selected::Extra(k));
                }
                egui::containers::menu::MenuButton::new("···").ui(ui, |ui| {
                    ui.set_min_width(260.0);
                    self.ways_menu(ui, k, sys);
                });
            });
            ui.end_row();
        }
    }

    /// Apps de Iurefficient para el teléfono, en la ficha de la cuenta.
    pub(super) fn mobile_section(&mut self, ui: &mut Ui) {
        section(ui, "App móvil");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            match MOBILE_IOS {
                Some(url) => {
                    let beta = url.contains("testflight");
                    let label = if beta {
                        "iPhone y iPad · TestFlight"
                    } else {
                        "iPhone y iPad · App Store"
                    };
                    if button(ui, label, Kind::Install)
                        .on_hover_text(url)
                        .clicked()
                    {
                        super::open_link(url);
                    }
                }
                None => {
                    ui.add_enabled_ui(false, |ui| {
                        button(ui, "iPhone y iPad · próximamente", Kind::Plain)
                    });
                }
            }
            match MOBILE_ANDROID {
                Some(url) => {
                    if button(ui, "Android · Google Play", Kind::Install)
                        .on_hover_text(url)
                        .clicked()
                    {
                        super::open_link(url);
                    }
                }
                None => {
                    ui.add_enabled_ui(false, |ui| {
                        button(ui, "Android · próximamente", Kind::Plain)
                    })
                    .response
                    .on_disabled_hover_text("Todavía no se publica en Google Play");
                }
            }
        });
        if MOBILE_IOS.is_some_and(|u| u.contains("testflight")) {
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "La app de iPhone y iPad está en beta: instala TestFlight desde la App Store \
                     y abre la liga en el teléfono o la tableta para unirte.",
                )
                .size(13.0)
                .color(pal().faint),
            );
        }
    }
}
