//! Dibujo de la ventana: cabecera, filtros, mosaico (o tabla) y barra de estado.

use super::theme::{self, BG, BRAND, ERROR, FAINT, INFO, LINE, MUTED, OK, SURFACE, TEXT, WARN};
use super::{Filter, LauncherApp, ViewMode};
use crate::catalog::{AppDef, Format, APPS};
use crate::plan::{self, Status};
use crate::system::{Os, SystemInfo};
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Align2, Color32, CornerRadius, FontId, Layout, Rect,
    RichText, Sense, Stroke, StrokeKind, TextFormat, Ui, UiBuilder,
};

const TILE_MIN_W: f32 = 330.0;
const TILE_H: f32 = 292.0;
const PAD: f32 = 20.0;

/// Texto en mayúsculas con espaciado, al estilo de las etiquetas de la interfaz.
fn caps(text: &str, size: f32, color: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: theme::mono(size),
            color,
            extra_letter_spacing: 1.2,
            ..Default::default()
        },
    );
    job
}

fn caps_label(ui: &mut Ui, text: &str, size: f32, color: Color32) -> egui::Response {
    ui.label(caps(text, size, color))
}

/// Botón pintado a mano: a diferencia de `egui::Button` con `fill`, conserva el cambio al
/// pasar el ratón y al pulsar (se oscurece y se hunde 1 px), con cursor de mano.
fn press_button(
    ui: &mut Ui,
    job: LayoutJob,
    fill: Color32,
    stroke: Stroke,
    radius: CornerRadius,
    min: egui::Vec2,
) -> egui::Response {
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let size = (galley.size() + 2.0 * ui.spacing().button_padding).max(min);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let hovered = enabled && resp.hovered();
        // Fondo claro: se oscurece; fondo transparente: se ilumina como el resto de la UI.
        let (bg, line) = if fill == Color32::TRANSPARENT {
            let v = &ui.visuals().widgets;
            match (pressed, hovered) {
                (true, _) => (v.active.weak_bg_fill, v.hovered.bg_stroke.color),
                (false, true) => (v.hovered.weak_bg_fill, v.hovered.bg_stroke.color),
                _ => (fill, stroke.color),
            }
        } else {
            let t = if pressed {
                0.28
            } else if hovered {
                0.14
            } else {
                0.0
            };
            (fill.lerp_to_gamma(BG, t), stroke.color)
        };
        let r = if pressed {
            rect.translate(vec2(0.0, 1.0)).shrink(0.5)
        } else {
            rect
        };
        // Deshabilitado, el pintor de `ui` ya lo atenúa.
        ui.painter().rect(
            r,
            radius,
            bg,
            Stroke::new(stroke.width, line),
            StrokeKind::Inside,
        );
        ui.painter()
            .galley(r.center() - galley.size() / 2.0, galley, TEXT);
    }
    if ui.is_enabled() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Botón plano con texto en mayúsculas; `fill` da el botón principal (fondo claro).
fn flat_button(ui: &mut Ui, text: &str, fill: Option<Color32>, min: egui::Vec2) -> egui::Response {
    let fg = if fill.is_some() { BG } else { TEXT };
    let radius = ui.visuals().widgets.inactive.corner_radius;
    match fill {
        Some(f) => press_button(ui, caps(text, 11.5, fg), f, Stroke::NONE, radius, min),
        None => press_button(
            ui,
            caps(text, 11.5, fg),
            Color32::TRANSPARENT,
            Stroke::new(1.0, LINE),
            radius,
            min,
        ),
    }
}

/// Grupo de opciones contiguas; devuelve el índice pulsado.
fn segmented(ui: &mut Ui, items: &[(String, bool)]) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (k, (label, selected)) in items.iter().enumerate() {
            let (fg, fill) = if *selected {
                (BG, TEXT)
            } else {
                (MUTED, Color32::TRANSPARENT)
            };
            let b = press_button(
                ui,
                caps(label, 11.0, fg),
                fill,
                Stroke::new(1.0, LINE),
                CornerRadius::ZERO,
                vec2(0.0, 30.0),
            );
            if b.clicked() {
                clicked = Some(k);
            }
        }
    });
    clicked
}

/// «2026-10-21» → «21 oct 2026».
fn nice_date(iso: &str) -> String {
    const M: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
    ];
    let mut p = iso.split('-');
    match (
        p.next(),
        p.next().and_then(|m| m.parse::<usize>().ok()),
        p.next(),
    ) {
        (Some(y), Some(m), Some(d)) if (1..=12).contains(&m) => {
            format!("{} {} {y}", d.trim_start_matches('0'), M[m - 1])
        }
        _ => iso.to_string(),
    }
}

/// Nombre a dos tonos: «Iure» en blanco y el resto en el color de la app.
fn two_tone(name: &str, size: f32, accent: Color32) -> LayoutJob {
    let split = name
        .char_indices()
        .nth(4)
        .map(|(i, _)| i)
        .unwrap_or(name.len());
    let mut job = LayoutJob::default();
    let fmt = |color| TextFormat {
        font_id: theme::bold_font(size),
        color,
        ..Default::default()
    };
    job.append(&name[..split], 0.0, fmt(TEXT));
    job.append(&name[split..], 0.0, fmt(accent));
    job
}

fn status_style(status: &Status) -> (Color32, &'static str) {
    match status {
        Status::NotInstalled => (INFO, "Disponible para instalar"),
        Status::UpToDate | Status::InstalledUnknown => (OK, "Instalada"),
        Status::UpdateAvailable => (WARN, "Actualización disponible"),
        Status::Checking => (MUTED, "Consultando…"),
        Status::Unavailable => (FAINT, "Sin versión publicada"),
        Status::Unsupported => (FAINT, "No disponible en este sistema"),
    }
}

fn chip(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.7)))
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.label(caps(text, 10.0, color));
        });
}

impl LauncherApp {
    fn matches(&self, i: usize) -> bool {
        let app = &APPS[i];
        let q = self.search.trim().to_lowercase();
        if !q.is_empty()
            && ![app.name, app.tagline, app.hint, app.description]
                .iter()
                .any(|t| t.to_lowercase().contains(&q))
        {
            return false;
        }
        let st = self.status(i);
        match self.filter {
            Filter::All => true,
            Filter::Installed => self.rows[i].installed.is_some(),
            Filter::Updates => st == Status::UpdateAvailable,
            Filter::NotInstalled => self.rows[i].installed.is_none(),
        }
    }

    fn count(&self, f: Filter) -> usize {
        (0..APPS.len())
            .filter(|&i| match f {
                Filter::All => true,
                Filter::Installed => self.rows[i].installed.is_some(),
                Filter::Updates => self.status(i) == Status::UpdateAvailable,
                Filter::NotInstalled => self.rows[i].installed.is_none(),
            })
            .count()
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(48.0);
            let narrow = ui.available_width() < 560.0;
            let mut brand = LayoutJob::default();
            brand.append(
                "IUREFFICIENT",
                0.0,
                TextFormat {
                    font_id: theme::bold_font(if narrow { 15.0 } else { 19.0 }),
                    color: TEXT,
                    extra_letter_spacing: if narrow { 1.2 } else { 2.5 },
                    ..Default::default()
                },
            );
            ui.label(brand);
            let full = ui.available_width() > 700.0;
            ui.add_space(10.0);
            if full {
                egui::Frame::new()
                    .fill(TEXT)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(caps("Lanzador", 10.5, BG));
                    });
            }
            if ui.available_width() > 640.0 {
                caps_label(ui, "Suite de escritorio", 10.5, FAINT);
            }
            let wide = ui.available_width() > 600.0;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if flat_button(ui, "Ajustes", None, vec2(0.0, 28.0)).clicked() {
                    self.show_settings = !self.show_settings;
                }
                if flat_button(ui, "Equipo", None, vec2(0.0, 28.0)).clicked() {
                    self.show_system = !self.show_system;
                }
                if flat_button(ui, "Comprobar", None, vec2(0.0, 28.0))
                    .on_hover_text("Buscar versiones nuevas en GitHub")
                    .clicked()
                {
                    self.refresh_releases();
                    for i in 0..APPS.len() {
                        self.redetect(i);
                    }
                }
                if !wide {
                    return;
                }
                ui.add_space(8.0);
                let pick = segmented(
                    ui,
                    &[
                        ("Tabla".into(), self.view == ViewMode::Table),
                        ("Mosaico".into(), self.view == ViewMode::Grid),
                    ],
                );
                if let Some(k) = pick {
                    self.view = if k == 0 {
                        ViewMode::Table
                    } else {
                        ViewMode::Grid
                    };
                    self.state.table_view = k == 0;
                    self.state.save();
                }
            });
        });
    }

    fn update_banner(&mut self, ui: &mut Ui) {
        let Some(r) = self.self_update.clone().filter(|_| !self.update_dismissed) else {
            return;
        };
        ui.add_space(14.0);
        egui::Frame::new()
            .fill(Color32::from_rgb(14, 22, 40))
            .stroke(Stroke::new(1.0, BRAND))
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(BRAND)
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(caps("Nuevo", 10.5, Color32::WHITE));
                        });
                    ui.label(
                        RichText::new(format!(
                            "Iurefficient Apps {} está disponible.",
                            r.version()
                        ))
                        .color(TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if flat_button(ui, "×", None, vec2(30.0, 30.0)).clicked() {
                            self.update_dismissed = true;
                        }
                        if flat_button(ui, "Descargar", Some(TEXT), vec2(110.0, 30.0)).clicked() {
                            super::open_link(&r.html_url);
                        }
                        if ui
                            .add(
                                egui::Label::new(caps("Novedades ↗", 10.5, MUTED))
                                    .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            super::open_link(&r.html_url);
                        }
                    });
                });
            });
    }

    fn filter_bar(&mut self, ui: &mut Ui) {
        ui.add_space(14.0);
        let narrow = ui.available_width() < 820.0;
        if narrow {
            self.search_and_update(ui, ui.available_width());
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            let filters = [
                (Filter::All, "Todas", "Todas"),
                (Filter::Installed, "Instaladas", "Instaladas"),
                (Filter::Updates, "Actualizaciones", "Nuevas"),
                (Filter::NotInstalled, "No instaladas", "Sin instalar"),
            ];
            let items: Vec<(String, bool)> = filters
                .iter()
                .map(|(f, long, short)| {
                    let l = if narrow { short } else { long };
                    (format!("{l}  {}", self.count(*f)), self.filter == *f)
                })
                .collect();
            if let Some(k) = segmented(ui, &items) {
                self.filter = filters[k].0;
            }
            if !narrow {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.search_and_update(ui, 220.0);
                });
            }
        });
    }

    /// Caja de búsqueda y «Actualizar todo» (si hay actualizaciones).
    fn search_and_update(&mut self, ui: &mut Ui, max_w: f32) {
        let size = vec2(ui.available_width(), 32.0);
        ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
            let pending: Vec<usize> = (0..APPS.len())
                .filter(|&i| {
                    self.status(i) == Status::UpdateAvailable && self.rows[i].busy.is_none()
                })
                .collect();
            let label = format!("Actualizar todo ({})", pending.len());
            let reserve = if pending.is_empty() { 0.0 } else { 190.0 };
            let w = (ui.available_width() - reserve).min(max_w).max(120.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Buscar apps")
                    .desired_width(w)
                    .margin(egui::Margin::symmetric(8, 7)),
            );
            if !pending.is_empty() && flat_button(ui, &label, Some(WARN), vec2(0.0, 30.0)).clicked()
            {
                for i in pending {
                    self.start_install(i);
                }
            }
        });
    }

    fn section_title(ui: &mut Ui, n: usize, title: &str, count: usize) {
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            caps_label(ui, &format!("{n:02} / {title}"), 11.0, MUTED);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let s = if count == 1 { "app" } else { "apps" };
                caps_label(ui, &format!("{count} {s}"), 11.0, FAINT);
            });
        });
        ui.add_space(10.0);
    }

    /// Mosaicos por sección; `None` es el de la cuenta de Iurefficient.
    fn sections(&self) -> Vec<(&'static str, Vec<Option<usize>>)> {
        let pick = |exp: bool| {
            (0..APPS.len())
                .filter(|&i| APPS[i].experimental == exp && self.matches(i))
                .map(Some)
                .collect::<Vec<_>>()
        };
        let mut suite = pick(false);
        if self.account_matches() {
            suite.insert(0, None);
        }
        vec![("Apps de la suite", suite), ("En exploración", pick(true))]
    }

    fn account_matches(&self) -> bool {
        let q = self.search.trim().to_lowercase();
        self.filter == Filter::All
            && (q.is_empty()
                || ["iurefficient", "cuenta", "instancia", "sesión"]
                    .iter()
                    .any(|t| t.contains(&q)))
    }

    fn grid(&mut self, ui: &mut Ui, sys: &SystemInfo) {
        let mut n = 0;
        for (title, items) in self.sections() {
            if items.is_empty() {
                continue;
            }
            n += 1;
            let apps = items.iter().filter(|i| i.is_some()).count();
            Self::section_title(ui, n, title, apps);
            let width = ui.available_width();
            let cols = ((width / TILE_MIN_W).floor() as usize).clamp(1, 4);
            let tile_w = (width / cols as f32).floor();
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for item in row {
                        match item {
                            Some(i) => self.tile(ui, *i, sys, tile_w),
                            None => self.account_tile(ui, tile_w),
                        }
                    }
                });
            }
        }
        if n == 0 {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Ninguna app coincide con el filtro.").color(MUTED));
            });
        }
    }

    /// Estado de la cuenta: (color, etiqueta).
    fn account_status(&self) -> (Color32, &'static str) {
        let a = &self.account;
        match &a.saved {
            _ if !a.loaded => (MUTED, "Consultando…"),
            Some(_) if a.busy => (MUTED, "Cerrando sesión…"),
            Some(_) if a.checking => (MUTED, "Comprobando sesión…"),
            Some(s) if s.has_session && !a.expired => (OK, "Conectada"),
            Some(_) => (WARN, "Sesión caducada"),
            None => (INFO, "Sin conectar"),
        }
    }

    fn account_connected(&self) -> bool {
        self.account.saved.as_ref().is_some_and(|s| s.has_session) && !self.account.expired
    }

    /// Mosaico de la cuenta de Iurefficient: conectar una vez para todas las apps y abrir
    /// la instancia en el navegador.
    fn account_tile(&mut self, ui: &mut Ui, w: f32) {
        let accent = INFO;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, TILE_H), Sense::hover());
        let painter = ui.painter_at(rect.expand(1.0));
        painter.rect(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgb(15, 21, 34),
            Stroke::new(1.0, LINE),
            StrokeKind::Inside,
        );
        if resp.hovered() || ui.rect_contains_pointer(rect) {
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
                0.0,
                accent,
            );
        }
        let strip = Rect::from_min_size(rect.min, vec2(w, 40.0));
        painter.hline(strip.x_range(), strip.bottom(), Stroke::new(1.0, LINE));
        let badge = Rect::from_min_size(strip.min + vec2(PAD, 10.0), vec2(30.0, 20.0));
        painter.rect_filled(badge, 0.0, BRAND);
        painter.text(
            badge.center(),
            Align2::CENTER_CENTER,
            "00",
            theme::mono(11.0),
            Color32::WHITE,
        );

        let connected = self.account_connected();
        let saved = self.account.saved.clone();
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(badge.right() + 8.0, strip.top()),
                    pos2(rect.right() - PAD, strip.bottom()),
                ))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                let gh = "https://github.com/ellaguno/iurefficient-connect";
                if ui
                    .add(egui::Label::new(caps("Conector ↗", 10.5, MUTED)).sense(Sense::click()))
                    .on_hover_text(gh)
                    .clicked()
                {
                    super::open_link(gh);
                }
                if let Some(s) = &saved {
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Label::new(caps("Instancia ↗", 10.5, MUTED))
                                .sense(Sense::click()),
                        )
                        .on_hover_text(&s.web_url)
                        .clicked()
                    {
                        super::open_link(&s.web_url);
                    }
                }
            },
        );

        let top = strip.bottom() + 20.0;
        let icon = Rect::from_min_size(pos2(rect.left() + PAD, top), vec2(60.0, 60.0));
        theme::paint_drawn_icon(&painter, icon, "iurefficient", accent);
        let (tagline, hint) = match (&saved, connected) {
            (Some(s), true) => (
                format!(
                    "Conectada como {}",
                    self.account.name.clone().unwrap_or_else(|| s.email.clone())
                ),
                format!("{} · {}", s.email, s.domain),
            ),
            (Some(s), false) => (
                "Vuelve a iniciar sesión".to_string(),
                format!("{} · {}", s.email, s.domain),
            ),
            (None, _) => (
                "Tu cuenta, para todas las apps".to_string(),
                "Inicia sesión una vez desde aquí".to_string(),
            ),
        };
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(icon.right() + 14.0, top - 4.0),
                    pos2(rect.right() - PAD, top + 70.0),
                ))
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(two_tone("Iurefficient", 25.0, accent));
                ui.add(egui::Label::new(RichText::new(tagline).size(14.5).color(TEXT)).truncate());
                ui.add(egui::Label::new(RichText::new(hint).size(12.5).color(FAINT)).truncate());
            },
        );

        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(rect.left() + PAD, top + 84.0),
                    pos2(rect.right() - PAD, rect.bottom() - 56.0),
                ))
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let (color, text) = self.account_status();
                chip(ui, text, color);
                let line = match &saved {
                    Some(s) if connected => {
                        let dav = if s.has_webdav {
                            "Sesión y WebDAV guardadas"
                        } else {
                            "Sesión guardada · sin WebDAV"
                        };
                        dav.to_string()
                    }
                    Some(s) if !s.app.is_empty() => format!("Cuenta configurada por {}", s.app),
                    _ => "IureDav, IureTranscribe, IureOCR e iureditor".to_string(),
                };
                ui.add(egui::Label::new(RichText::new(line).size(13.0).color(MUTED)).truncate());
                let note = match &self.account.error {
                    Some(e) => RichText::new(e).size(12.0).color(ERROR),
                    None => RichText::new("Las apps sin cuenta propia la toman al abrirse")
                        .size(12.0)
                        .color(FAINT),
                };
                ui.add(egui::Label::new(note).truncate());
            },
        );

        let bar = Rect::from_min_max(
            pos2(rect.left() + PAD, rect.bottom() - 54.0),
            pos2(rect.right() - PAD, rect.bottom() - 18.0),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                self.account_actions(ui, bar.width());
            },
        );
    }

    fn account_actions(&mut self, ui: &mut Ui, width: f32) {
        let size = vec2(width - 44.0, 36.0);
        let busy = !self.account.loaded || self.account.busy || self.account.checking;
        let saved = self.account.saved.clone();
        if self.account_connected() {
            if let Some(s) = &saved {
                if flat_button(ui, "Abrir Iurefficient", Some(TEXT), size)
                    .on_hover_text(&s.web_url)
                    .clicked()
                {
                    super::open_link(&s.web_url);
                }
            }
        } else {
            let label = if saved.is_some() {
                "Volver a conectar"
            } else {
                "Conectar"
            };
            let r = ui
                .add_enabled_ui(!busy, |ui| flat_button(ui, label, Some(TEXT), size))
                .inner;
            if r.clicked() {
                self.open_login();
            }
        }
        ui.add_space(8.0);
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = vec2(0.0, 0.0);
            // Transparente en reposo, sin perder el realce al pasar el ratón y al pulsar.
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            let menu = egui::Button::new(RichText::new("···").size(15.0).color(TEXT))
                .min_size(vec2(36.0, 36.0));
            egui::containers::menu::MenuButton::from_button(menu).ui(ui, |ui| {
                ui.set_min_width(210.0);
                if let Some(s) = &saved {
                    if ui.button("Abrir la instancia en el navegador").clicked() {
                        super::open_link(&s.web_url);
                    }
                }
                if ui
                    .button(if saved.is_some() {
                        "Cambiar de cuenta…"
                    } else {
                        "Conectar…"
                    })
                    .clicked()
                {
                    self.open_login();
                }
                if saved.is_some() && !busy {
                    ui.separator();
                    if ui
                        .button(RichText::new("Cerrar sesión…").color(ERROR))
                        .clicked()
                    {
                        self.confirm_logout = true;
                    }
                }
            });
        });
    }

    fn login_modal(&mut self, ctx: &egui::Context) {
        if self.login.is_none() {
            return;
        }
        let mut submit = false;
        let mut cancel = false;
        let resp = egui::Modal::new(egui::Id::unique("iureapps-login")).show(ctx, |ui| {
            let Some(f) = self.login.as_mut() else { return };
            ui.set_width(380.0);
            ui.label(RichText::new("Conectar con Iurefficient").font(theme::bold_font(18.0)));
            ui.add_space(4.0);
            ui.label(
                RichText::new("La sesión se guarda en el llavero del sistema y la comparten IureDav, IureTranscribe, IureOCR e iureditor.")
                    .size(12.5)
                    .color(MUTED),
            );
            ui.add_space(12.0);
            let field = |ui: &mut Ui, label: &str, value: &mut String, hint: &str, password: bool, enabled: bool| {
                caps_label(ui, label, 10.0, FAINT);
                let r = ui.add_enabled(
                    enabled,
                    egui::TextEdit::singleline(value)
                        .hint_text(hint)
                        .password(password)
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(8, 7)),
                );
                ui.add_space(6.0);
                r
            };
            let editing = !f.busy && f.totp_token.is_none();
            let d = field(ui, "Instancia", &mut f.domain, "2.ds.iurefficient.com", false, editing);
            let e = field(ui, "Correo", &mut f.email, "tu@despacho.com", false, editing);
            let pw = field(ui, "Contraseña", &mut f.password, "", true, editing);
            if !f.focused {
                f.focused = true;
                if f.domain.is_empty() {
                    d.request_focus();
                } else if f.email.is_empty() {
                    e.request_focus();
                } else {
                    pw.request_focus();
                }
            }
            if pw.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            if f.totp_token.is_some() {
                let code = field(ui, "Código de verificación", &mut f.code, "123456", false, !f.busy);
                if code.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if !f.busy && !code.has_focus() && f.code.is_empty() {
                    code.request_focus();
                }
            }
            if let Some(e) = &f.error {
                ui.label(RichText::new(e).size(12.5).color(ERROR));
                ui.add_space(6.0);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ready = !f.busy
                    && !f.domain.trim().is_empty()
                    && !f.email.trim().is_empty()
                    && (!f.password.is_empty() || f.totp_token.is_some());
                let label = if f.totp_token.is_some() { "Verificar" } else { "Conectar" };
                if ui.add_enabled_ui(ready, |ui| flat_button(ui, label, Some(TEXT), vec2(130.0, 32.0))).inner.clicked() {
                    submit = true;
                }
                if flat_button(ui, "Cancelar", None, vec2(100.0, 32.0)).clicked() {
                    cancel = true;
                }
                if f.busy {
                    ui.spinner();
                }
            });
        });
        if submit {
            self.submit_login();
        }
        if cancel || (resp.should_close() && !self.login.as_ref().is_some_and(|f| f.busy)) {
            self.login = None;
        }
    }

    fn logout_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_logout {
            return;
        }
        let resp = egui::Modal::new(egui::Id::unique("iureapps-logout")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new("¿Cerrar la sesión de Iurefficient?").font(theme::bold_font(18.0)));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Se cierra en la instancia y en este equipo. Las apps abiertas siguen conectadas hasta que las cierres; la contraseña WebDAV se conserva.")
                    .color(MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if flat_button(ui, "Cerrar sesión", Some(ERROR), vec2(140.0, 32.0)).clicked() {
                    self.logout();
                }
                if flat_button(ui, "Cancelar", None, vec2(100.0, 32.0)).clicked() {
                    self.confirm_logout = false;
                }
            });
        });
        if resp.should_close() {
            self.confirm_logout = false;
        }
    }

    fn tile(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo, w: f32) {
        let app: &'static AppDef = &APPS[i];
        let accent = theme::accent(app.accent);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, TILE_H), Sense::hover());
        let painter = ui.painter_at(rect.expand(1.0));
        let hovered = resp.hovered() || ui.rect_contains_pointer(rect);
        painter.rect(
            rect,
            CornerRadius::ZERO,
            SURFACE,
            Stroke::new(1.0, LINE),
            StrokeKind::Inside,
        );
        if hovered {
            painter.rect_filled(
                Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)),
                0.0,
                accent,
            );
        }

        // Franja superior: número y enlaces.
        let strip = Rect::from_min_size(rect.min, vec2(w, 40.0));
        painter.hline(strip.x_range(), strip.bottom(), Stroke::new(1.0, LINE));
        let badge = Rect::from_min_size(strip.min + vec2(PAD, 10.0), vec2(30.0, 20.0));
        painter.rect_filled(badge, 0.0, accent);
        painter.text(
            badge.center(),
            Align2::CENTER_CENTER,
            format!("{:02}", i + 1),
            theme::mono(11.0),
            BG,
        );

        let release = self.rows[i]
            .release
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .cloned();
        let installed = self.rows[i].installed.clone();
        let status = self.status(i);

        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect.shrink2(vec2(PAD, 0.0)))
                .layout(Layout::top_down(Align::Min)),
        );
        // Enlaces a la derecha de la franja.
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(badge.right() + 8.0, strip.top()),
                    pos2(rect.right() - PAD, strip.bottom()),
                ))
                .layout(Layout::right_to_left(Align::Center)),
            |ui| {
                let gh = format!("https://github.com/{}", app.repo);
                if ui
                    .add(egui::Label::new(caps("GitHub ↗", 10.5, MUTED)).sense(Sense::click()))
                    .on_hover_text(&gh)
                    .clicked()
                {
                    super::open_link(gh);
                }
                ui.add_space(10.0);
                let notes_url = release
                    .as_ref()
                    .map(|r| r.html_url.clone())
                    .unwrap_or_else(|| app.release_url());
                let notes = release
                    .as_ref()
                    .and_then(|r| r.body.clone())
                    .unwrap_or_default();
                let link = ui
                    .add(egui::Label::new(caps("Novedades ↗", 10.5, MUTED)).sense(Sense::click()));
                let link = if notes.trim().is_empty() {
                    link
                } else {
                    link.on_hover_text(notes.chars().take(900).collect::<String>())
                };
                if link.clicked() {
                    super::open_link(notes_url);
                }
            },
        );

        // Icono, nombre y descripción.
        let top = strip.bottom() + 20.0;
        let icon = Rect::from_min_size(pos2(rect.left() + PAD, top), vec2(60.0, 60.0));
        self.icons.paint(&painter, icon, app.id, accent);
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(icon.right() + 14.0, top - 4.0),
                    pos2(rect.right() - PAD, top + 70.0),
                ))
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(two_tone(app.name, 25.0, accent))
                    .on_hover_text(app.description);
                ui.add(
                    egui::Label::new(RichText::new(app.tagline).size(14.5).color(TEXT)).truncate(),
                );
                ui.add(
                    egui::Label::new(RichText::new(app.hint).size(12.5).color(FAINT)).truncate(),
                );
            },
        );

        // Estado y versión.
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    pos2(rect.left() + PAD, top + 84.0),
                    pos2(rect.right() - PAD, rect.bottom() - 56.0),
                ))
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let (color, text) = status_style(&status);
                let text = match (&status, &release) {
                    (Status::UpdateAvailable, Some(r)) => format!("Actualización {}", r.version()),
                    _ => text.to_string(),
                };
                chip(ui, &text, color);
                let version = match (&installed, &release) {
                    (Some(inst), _) => {
                        let mut s = format!(
                            "Versión {} · {}",
                            inst.version.as_deref().unwrap_or("?"),
                            inst.format.label()
                        );
                        if let Some(v) = inst.variant.filter(|_| app.gpu_variants) {
                            s.push_str(&format!(" · {}", v.label()));
                        }
                        s
                    }
                    (None, Some(r)) => format!("Versión {} · {}", r.version(), nice_date(r.date())),
                    (None, None) => match &self.rows[i].release {
                        Some(Err(_)) if status == Status::Unavailable && self.token().is_some() => {
                            "Todavía no hay versiones en GitHub".into()
                        }
                        Some(Err(e)) => e.clone(),
                        _ => "Consultando GitHub…".into(),
                    },
                };
                ui.add(egui::Label::new(RichText::new(version).size(13.0).color(MUTED)).truncate());
                self.tile_options(ui, i, sys);
            },
        );

        // Acciones.
        let bar = Rect::from_min_max(
            pos2(rect.left() + PAD, rect.bottom() - 54.0),
            pos2(rect.right() - PAD, rect.bottom() - 18.0),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::left_to_right(Align::Center)),
            |ui| {
                self.tile_actions(ui, i, sys, bar.width());
            },
        );
    }

    /// Variante (IureTranscribe), formato y avisos, en una línea fija del mosaico.
    fn tile_options(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo) {
        let app: &'static AppDef = &APPS[i];
        if let Some(e) = self.rows[i].error.clone() {
            ui.add(egui::Label::new(RichText::new(&e).size(12.0).color(ERROR)).truncate())
                .on_hover_text(e);
            return;
        }
        let Some(Ok(release)) = self.rows[i].release.clone() else {
            if !app.note.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(app.note).size(12.0).color(FAINT)).truncate(),
                );
            }
            return;
        };
        if !app.supports(sys) {
            if !app.note.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(app.note).size(12.0).color(FAINT)).truncate(),
                );
            }
            return;
        }
        let installed = self.rows[i].installed.clone();
        let pref = self.state.prefs.get(app.id).cloned();
        let variant = plan::variant(app, sys, installed.as_ref(), pref.as_ref());
        let formats: Vec<Format> = app
            .formats(sys)
            .into_iter()
            .filter(|f| app.select_asset(&release, sys, variant, *f).is_some())
            .collect();
        let current = plan::format(
            app,
            sys,
            &release,
            variant,
            installed.as_ref(),
            pref.as_ref(),
        );
        let mut new_variant = variant;
        let mut new_format = None;
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            if app.gpu_variants {
                let (recommended, why) = app.recommend_variant(sys);
                caps_label(ui, "Aceleración", 10.0, FAINT);
                let selected = if variant == recommended {
                    RichText::new(format!("{} · recomendada", variant.label())).size(12.5)
                } else {
                    RichText::new(variant.label()).size(12.5)
                };
                egui::ComboBox::from_id_salt(("variant", i))
                    .selected_text(selected)
                    .width(176.0)
                    .show_ui(ui, |ui| {
                        for v in app.variants(sys) {
                            let ok = app.best_format(&release, sys, v).is_some();
                            let mut text = v.label().to_string();
                            if v == recommended {
                                text.push_str("  · recomendada");
                            }
                            ui.add_enabled_ui(ok, |ui| {
                                ui.selectable_value(&mut new_variant, v, text)
                            });
                        }
                    })
                    .response
                    .on_hover_text(format!("Recomendada: {}. {why}", recommended.label()));
            }
            if let Some(cur) = current {
                if formats.len() > 1 {
                    if !app.gpu_variants {
                        caps_label(ui, "Formato", 10.0, FAINT);
                    }
                    let mut f = cur;
                    egui::ComboBox::from_id_salt(("format", i))
                        .selected_text(RichText::new(f.label()).size(12.5))
                        .width(92.0)
                        .show_ui(ui, |ui| {
                            for x in &formats {
                                ui.selectable_value(&mut f, *x, x.label());
                            }
                        });
                    if f != cur {
                        new_format = Some(f);
                    }
                } else if app.gpu_variants {
                    ui.label(RichText::new(cur.label()).size(12.0).color(FAINT));
                } else if !app.note.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(app.note).size(12.0).color(FAINT))
                            .truncate(),
                    );
                }
            }
        });
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

    /// ¿La selección (variante/formato) difiere de lo instalado?
    fn selection_differs(&self, i: usize, sys: &SystemInfo) -> bool {
        let app = &APPS[i];
        let (Some(inst), Some(Ok(release))) = (&self.rows[i].installed, &self.rows[i].release)
        else {
            return false;
        };
        let pref = self.state.prefs.get(app.id);
        let v = plan::variant(app, sys, Some(inst), pref);
        let f = plan::format(app, sys, release, v, Some(inst), pref);
        (app.gpu_variants && inst.variant.is_some_and(|iv| iv != v))
            || f.is_some_and(|f| f != inst.format)
    }

    fn tile_actions(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo, width: f32) {
        let app: &'static AppDef = &APPS[i];
        if let Some(text) = self.rows[i].busy.clone() {
            let p = self.rows[i].progress.unwrap_or(0.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.add(
                    egui::ProgressBar::new(p)
                        .desired_width(width)
                        .desired_height(8.0)
                        .fill(theme::accent(app.accent))
                        .corner_radius(0.0),
                );
                ui.horizontal(|ui| {
                    ui.spinner();
                    caps_label(ui, &format!("{text} {:.0}%", p * 100.0), 10.0, MUTED);
                });
            });
            return;
        }
        let status = self.status(i);
        let has_release = matches!(self.rows[i].release, Some(Ok(_))) && app.supports(sys);
        let installed = self.rows[i].installed.is_some();
        let differs = self.selection_differs(i, sys);
        let main_w = width - 44.0;
        let size = vec2(main_w, 36.0);
        match status {
            Status::NotInstalled if has_release => {
                if flat_button(ui, "Instalar", Some(TEXT), size).clicked() {
                    self.start_install(i);
                }
            }
            Status::UpdateAvailable if has_release => {
                if flat_button(ui, "Actualizar", Some(WARN), size).clicked() {
                    self.start_install(i);
                }
            }
            _ if installed && differs && has_release => {
                if flat_button(ui, "Cambiar a la selección", Some(TEXT), size).clicked() {
                    self.start_install(i);
                }
            }
            _ if installed => {
                if flat_button(ui, "Abrir", Some(TEXT), size).clicked() {
                    self.launch(i);
                }
            }
            _ => {
                ui.add_enabled_ui(false, |ui| flat_button(ui, "No disponible", None, size));
            }
        }
        ui.add_space(8.0);
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = vec2(0.0, 0.0);
            // Transparente en reposo, sin perder el realce al pasar el ratón y al pulsar.
            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
            let menu = egui::Button::new(RichText::new("···").size(15.0).color(TEXT))
                .min_size(vec2(36.0, 36.0));
            egui::containers::menu::MenuButton::from_button(menu).ui(ui, |ui| {
                ui.set_min_width(200.0);
                if installed && status != Status::UpToDate && ui.button("Abrir").clicked() {
                    self.launch(i);
                }
                if installed && has_release && ui.button("Reinstalar").clicked() {
                    self.start_install(i);
                }
                let notes = self.rows[i]
                    .release
                    .as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .map(|r| r.html_url.clone())
                    .unwrap_or_else(|| app.release_url());
                if ui.button("Novedades de la versión").clicked() {
                    super::open_link(notes);
                }
                if ui.button("Todas las versiones").clicked() {
                    super::open_link(app.release_url());
                }
                if installed {
                    ui.separator();
                    if ui
                        .button(RichText::new("Desinstalar…").color(ERROR))
                        .clicked()
                    {
                        self.confirm_uninstall = Some(i);
                    }
                }
            });
        });
    }

    fn table(&mut self, ui: &mut Ui, sys: &SystemInfo) {
        ui.add_space(18.0);
        let items: Vec<usize> = (0..APPS.len()).filter(|&i| self.matches(i)).collect();
        egui::Frame::new()
            .stroke(Stroke::new(1.0, LINE))
            .fill(SURFACE)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("apps-table")
                    .num_columns(6)
                    .spacing([18.0, 10.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for h in ["", "App", "Estado", "Instalada", "Última", ""] {
                            caps_label(ui, h, 10.0, FAINT);
                        }
                        ui.end_row();
                        if self.account_matches() {
                            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                            theme::paint_drawn_icon(ui.painter(), r, "iurefficient", INFO);
                            ui.vertical(|ui| {
                                ui.set_min_width(230.0);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.add(
                                    egui::Label::new(two_tone("Iurefficient", 15.0, INFO)).extend(),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new("Cuenta para todas las apps")
                                            .size(12.0)
                                            .color(FAINT),
                                    )
                                    .extend(),
                                );
                            });
                            let (c, t) = self.account_status();
                            ui.label(caps(t, 10.0, c));
                            let who = self
                                .account
                                .saved
                                .as_ref()
                                .map(|s| s.email.clone())
                                .unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(who).color(MUTED));
                            let dom = self
                                .account
                                .saved
                                .as_ref()
                                .map(|s| s.domain.clone())
                                .unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(dom).color(MUTED));
                            ui.horizontal(|ui| self.account_actions(ui, 190.0));
                            ui.end_row();
                        }
                        for i in items {
                            let app = &APPS[i];
                            let accent = theme::accent(app.accent);
                            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                            self.icons.paint(ui.painter(), r, app.id, accent);
                            ui.vertical(|ui| {
                                ui.set_min_width(230.0);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.add(egui::Label::new(two_tone(app.name, 15.0, accent)).extend());
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(app.tagline).size(12.0).color(FAINT),
                                    )
                                    .extend(),
                                );
                            });
                            let st = self.status(i);
                            let (c, t) = status_style(&st);
                            ui.label(caps(t, 10.0, c));
                            let inst = self.rows[i]
                                .installed
                                .as_ref()
                                .map(|x| {
                                    format!(
                                        "{} · {}",
                                        x.version.as_deref().unwrap_or("?"),
                                        x.format.label()
                                    )
                                })
                                .unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(inst).color(MUTED));
                            let latest = match &self.rows[i].release {
                                Some(Ok(r)) => format!("{} · {}", r.version(), nice_date(r.date())),
                                Some(Err(_)) => "—".into(),
                                None => "…".into(),
                            };
                            ui.label(RichText::new(latest).color(MUTED));
                            ui.horizontal(|ui| self.tile_actions(ui, i, sys, 190.0));
                            ui.end_row();
                        }
                    });
            });
    }

    fn footer(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.set_min_height(30.0);
            let checked = match self.last_checked {
                None => "Consultando GitHub…".to_string(),
                Some(t) => {
                    let m = t.elapsed().as_secs() / 60;
                    match m {
                        0 => "Consultado hace un momento".into(),
                        1 => "Consultado hace 1 min".into(),
                        _ => format!("Consultado hace {m} min"),
                    }
                }
            };
            let source = if ui.available_width() > 560.0 {
                " · GitHub Releases"
            } else {
                ""
            };
            caps_label(ui, &format!("{checked}{source}"), 10.0, FAINT);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = format!("Registro ({})", self.log.len());
                if ui
                    .add(egui::Label::new(caps(&label, 10.0, MUTED)).sense(Sense::click()))
                    .clicked()
                {
                    self.show_log = !self.show_log;
                }
                ui.add_space(14.0);
                if ui.available_width() < 520.0 {
                    return;
                }
                if let Some(sys) = &self.sys {
                    let arch = match sys.arch {
                        crate::system::Arch::X64 => "x64",
                        crate::system::Arch::Arm64 => "arm64",
                        crate::system::Arch::Other => std::env::consts::ARCH,
                    };
                    let os = match sys.os {
                        Os::Linux => "Linux",
                        Os::Windows => "Windows",
                        Os::MacOs => "macOS",
                        Os::Other => std::env::consts::OS,
                    };
                    caps_label(ui, &format!("Instalando para {os} {arch}"), 10.0, FAINT);
                }
            });
        });
    }

    fn system_window(&mut self, ctx: &egui::Context) {
        let Some(sys) = self.sys.clone() else { return };
        let mut open = self.show_system;
        egui::Window::new(RichText::new("Este equipo").font(theme::bold_font(16.0)))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(460.0)
            .show(ctx, |ui| {
                egui::Grid::new("sys")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        let mut row = |k: &str, v: String| {
                            caps_label(ui, k, 10.0, FAINT);
                            ui.label(RichText::new(v).color(TEXT));
                            ui.end_row();
                        };
                        row("Sistema", sys.platform_label());
                        let avx = match sys.avx2 {
                            Some(true) => " · AVX2",
                            Some(false) => " · sin AVX2 (Whisper irá lento)",
                            None => "",
                        };
                        row("CPU", format!("{} · {} hilos{avx}", sys.cpu, sys.cores));
                        if let Some(r) = sys.ram_gb {
                            row("Memoria", format!("{r:.1} GB"));
                        }
                        row(
                            "GPU",
                            if sys.gpus.is_empty() {
                                "no detectada".into()
                            } else {
                                sys.gpus
                                    .iter()
                                    .map(|g| g.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            },
                        );
                        row(
                            "CUDA",
                            match &sys.nvidia {
                                Some(n) => format!(
                                    "driver NVIDIA {}{}",
                                    n.driver,
                                    n.vram_mb
                                        .map(|m| format!(" · {:.1} GB VRAM", m as f64 / 1024.0))
                                        .unwrap_or_default()
                                ),
                                None => "no".into(),
                            },
                        );
                        row(
                            "Vulkan",
                            if sys.vulkan {
                                "sí".into()
                            } else {
                                "no".into()
                            },
                        );
                        if sys.os == Os::Linux {
                            row("Paquetes", format!("{:?}", sys.pkg));
                            row(
                                "AppImage",
                                if sys.fuse {
                                    "con FUSE".into()
                                } else {
                                    "sin FUSE: se extrae al abrir".into()
                                },
                            );
                            if !sys.desktop.is_empty() {
                                row("Escritorio", format!("{} · {}", sys.desktop, sys.session));
                            }
                        }
                        if let Some(app) = crate::catalog::find("iuretranscribe") {
                            let (v, why) = app.recommend_variant(&sys);
                            row("IureTranscribe", format!("{} — {why}", v.label()));
                        }
                    });
            });
        self.show_system = open;
    }

    fn log_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_log;
        egui::Window::new(RichText::new("Registro").font(theme::bold_font(16.0)))
            .open(&mut open)
            .default_size([560.0, 260.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.log.is_empty() {
                            ui.label(RichText::new("Sin actividad todavía.").color(FAINT));
                        }
                        for l in &self.log {
                            ui.label(RichText::new(l).font(FontId::monospace(11.5)).color(MUTED));
                        }
                    });
            });
        self.show_log = open;
    }

    fn uninstall_modal(&mut self, ctx: &egui::Context) {
        let Some(i) = self.confirm_uninstall else {
            return;
        };
        let app = &APPS[i];
        let resp = egui::Modal::new(egui::Id::unique("iureapps-uninstall")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(
                RichText::new(format!("¿Desinstalar {}?", app.name)).font(theme::bold_font(18.0)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Se quita la app; tus documentos y ajustes no se tocan.")
                    .color(MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if flat_button(ui, "Desinstalar", Some(ERROR), vec2(130.0, 32.0)).clicked() {
                    self.start_uninstall(i);
                }
                if flat_button(ui, "Cancelar", None, vec2(100.0, 32.0)).clicked() {
                    self.confirm_uninstall = None;
                }
            });
        });
        if resp.should_close() {
            self.confirm_uninstall = None;
        }
    }
}

impl eframe::App for LauncherApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_messages();
        let ctx = ui.ctx().clone();
        // Para que «Consultado hace…» avance sin interacción.
        ctx.request_repaint_after(std::time::Duration::from_secs(30));

        let bar = egui::Frame::new()
            .fill(BG)
            .inner_margin(egui::Margin::symmetric(28, 6));
        egui::Panel::top("header")
            .frame(bar)
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("footer")
            .frame(bar)
            .show(ui, |ui| self.footer(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin {
                left: 28,
                right: 28,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| {
                // Líneas de separación de cabecera y pie, de borde a borde.
                let full = ui.max_rect().expand2(vec2(28.0, 0.0));
                ui.painter()
                    .hline(full.x_range(), full.top(), Stroke::new(1.0, LINE));
                ui.painter()
                    .hline(full.x_range(), full.bottom(), Stroke::new(1.0, LINE));
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Hueco para la barra de desplazamiento.
                        ui.set_max_width(ui.available_width() - 14.0);
                        self.update_banner(ui);
                        self.filter_bar(ui);
                        match self.sys.clone() {
                            None => {
                                ui.add_space(40.0);
                                ui.vertical_centered(|ui| {
                                    ui.spinner();
                                    ui.label(RichText::new("Analizando el equipo…").color(MUTED));
                                });
                            }
                            Some(sys) => match self.view {
                                ViewMode::Grid => self.grid(ui, &sys),
                                ViewMode::Table => self.table(ui, &sys),
                            },
                        }
                        ui.add_space(24.0);
                    });
            });
        if self.show_settings {
            self.settings_window(&ctx);
        }
        if self.show_system {
            self.system_window(&ctx);
        }
        if self.show_log {
            self.log_window(&ctx);
        }
        self.uninstall_modal(&ctx);
        self.login_modal(&ctx);
        self.logout_modal(&ctx);
    }
}
