//! Dibujo de la ventana: cabecera, filtros, mosaico (o tabla) y barra de estado.

use super::theme::{self, pal};
use super::{Filter, LauncherApp, Selected, ViewMode};
use crate::catalog::{AppDef, Format, APPS};
use crate::plan::{self, Status};
use crate::system::{Os, SystemInfo};
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Color32, CornerRadius, FontId, Layout, Rect,
    RichText, Sense, Stroke, StrokeKind, TextFormat, Ui,
};

const CELL_W: f32 = 136.0;
const CELL_H: f32 = 156.0;
const CELL_ICON: f32 = 72.0;

/// Distintivo de estado en la esquina del icono.
#[derive(Clone, Copy)]
pub(super) enum Badge {
    /// Instalada (✓ verde).
    Ok,
    /// Actualización (↑ ámbar).
    Update,
    /// Disponible para instalar (↓ azul).
    Get,
    /// No disponible (⊘ gris).
    No,
    /// Hay que atender algo (! ámbar).
    Warn,
}

/// Lo que pinta una celda del mosaico.
struct Cell {
    icon_id: &'static str,
    accent: Color32,
    name: LayoutJob,
    /// Texto bajo el nombre y su color (estado o para qué sirve la app).
    line: (String, Color32),
    badge: Option<Badge>,
    /// Icono atenuado: no se puede instalar.
    dim: bool,
    /// Barra de avance en lugar del texto, mientras instala.
    progress: Option<f32>,
}

/// Acción principal de una app.
#[derive(Clone, Copy)]
pub(super) enum Action {
    Open,
    Install,
    Update,
}

/// Círculo de color con un glifo dibujado (no depende de que la fuente lo tenga) y un aro
/// del color de fondo para separarlo del icono.
pub(super) fn paint_badge(
    painter: &egui::Painter,
    c: egui::Pos2,
    r: f32,
    badge: Badge,
    bg: Color32,
) {
    let fill = match badge {
        Badge::Ok => pal().ok,
        Badge::Update | Badge::Warn => pal().warn,
        Badge::Get => pal().info,
        Badge::No => pal().faint,
    };
    painter.circle(c, r, fill, Stroke::new(r * 0.24, bg));
    let s = Stroke::new((r * 0.18).max(1.6), Color32::WHITE);
    let k = r * 0.45;
    match badge {
        Badge::Ok => {
            painter.line(
                vec![
                    c + vec2(-k, 0.0),
                    c + vec2(-k * 0.3, k * 0.7),
                    c + vec2(k, -k * 0.6),
                ],
                s,
            );
        }
        Badge::Update | Badge::Get => {
            let dir = if matches!(badge, Badge::Update) {
                -1.0
            } else {
                1.0
            };
            painter.line_segment([c + vec2(0.0, -k * dir), c + vec2(0.0, k * dir)], s);
            painter.line(
                vec![
                    c + vec2(-k * 0.75, k * 0.1 * dir),
                    c + vec2(0.0, k * dir),
                    c + vec2(k * 0.75, k * 0.1 * dir),
                ],
                s,
            );
        }
        Badge::No => {
            painter.circle_stroke(c, k, s);
            painter.line_segment(
                [c + vec2(-k * 0.7, k * 0.7), c + vec2(k * 0.7, -k * 0.7)],
                s,
            );
        }
        Badge::Warn => {
            painter.line_segment([c + vec2(0.0, -k), c + vec2(0.0, k * 0.25)], s);
            painter.circle_filled(c + vec2(0.0, k * 0.8), s.width * 0.6, Color32::WHITE);
        }
    }
}

/// Texto en mayúsculas con espaciado, al estilo de las etiquetas de la interfaz.
pub(super) fn caps(text: &str, size: f32, color: Color32) -> LayoutJob {
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

pub(super) fn caps_label(ui: &mut Ui, text: &str, size: f32, color: Color32) -> egui::Response {
    ui.label(caps(text, size, color))
}

/// Botón pintado a mano: a diferencia de `egui::Button` con `fill`, conserva el cambio al
/// pasar el ratón y al pulsar (se oscurece y se hunde 1 px), con cursor de mano.
pub(super) fn press_button(
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
            (fill.lerp_to_gamma(pal().bg, t), stroke.color)
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
            .galley(r.center() - galley.size() / 2.0, galley, pal().text);
    }
    if ui.is_enabled() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Botón plano con texto en mayúsculas; `fill` da el botón principal (fondo claro).
pub(super) fn flat_button(
    ui: &mut Ui,
    text: &str,
    fill: Option<Color32>,
    min: egui::Vec2,
) -> egui::Response {
    let fg = if fill.is_some() { pal().bg } else { pal().text };
    let radius = ui.visuals().widgets.inactive.corner_radius;
    match fill {
        Some(f) => press_button(ui, caps(text, 11.5, fg), f, Stroke::NONE, radius, min),
        None => press_button(
            ui,
            caps(text, 11.5, fg),
            Color32::TRANSPARENT,
            Stroke::new(1.0, pal().line),
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
                (pal().bg, pal().text)
            } else {
                (pal().muted, Color32::TRANSPARENT)
            };
            let b = press_button(
                ui,
                caps(label, 11.0, fg),
                fill,
                Stroke::new(1.0, pal().line),
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
pub(super) fn nice_date(iso: &str) -> String {
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
pub(super) fn two_tone(name: &str, size: f32, accent: Color32) -> LayoutJob {
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
    job.append(&name[..split], 0.0, fmt(pal().text));
    job.append(&name[split..], 0.0, fmt(accent));
    job
}

pub(super) fn status_style(status: &Status) -> (Color32, &'static str) {
    match status {
        Status::NotInstalled => (pal().info, "Disponible para instalar"),
        Status::UpToDate | Status::InstalledUnknown => (pal().ok, "Instalada"),
        Status::UpdateAvailable => (pal().warn, "Actualización disponible"),
        Status::Checking => (pal().muted, "Consultando…"),
        Status::Unavailable => (pal().faint, "Sin versión publicada"),
        Status::Unsupported => (pal().faint, "No disponible en este sistema"),
    }
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
            self.icons.logo(ui, if narrow { 18.0 } else { 24.0 });
            let full = ui.available_width() > 700.0;
            ui.add_space(10.0);
            if full {
                egui::Frame::new()
                    .fill(pal().text)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(caps("Lanzador", 10.5, pal().bg));
                    });
            }
            if ui.available_width() > 640.0 {
                caps_label(ui, "Suite de escritorio", 10.5, pal().faint);
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
                    self.shots.clear();
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
                    self.selected = None;
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
            .fill(pal().banner)
            .stroke(Stroke::new(1.0, pal().brand))
            .inner_margin(egui::Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(pal().brand)
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.label(caps("Nuevo", 10.5, Color32::WHITE));
                        });
                    ui.label(
                        RichText::new(format!(
                            "Iurefficient Apps {} está disponible.",
                            r.version()
                        ))
                        .color(pal().text),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(super::SelfUpdate::Running(p)) = self.self_updating {
                            ui.spinner();
                            caps_label(
                                ui,
                                &format!("Actualizando… {:.0}%", p * 100.0),
                                10.5,
                                pal().muted,
                            );
                            ui.add(
                                egui::ProgressBar::new(p)
                                    .desired_width(160.0)
                                    .desired_height(6.0)
                                    .fill(pal().brand)
                                    .corner_radius(0.0),
                            );
                            return;
                        }
                        if flat_button(ui, "×", None, vec2(30.0, 30.0)).clicked() {
                            self.update_dismissed = true;
                        }
                        let plan = self
                            .sys
                            .as_ref()
                            .map(|sys| crate::selfupdate::plan(&r, sys).map(|_| ()));
                        let failed =
                            matches!(self.self_updating, Some(super::SelfUpdate::Failed(_)));
                        match plan {
                            Some(Ok(())) if !failed => {
                                let busy = self.rows.iter().any(|r| r.busy.is_some());
                                let b = ui
                                    .add_enabled_ui(!busy, |ui| {
                                        flat_button(
                                            ui,
                                            "Actualizar y reiniciar",
                                            Some(pal().text),
                                            vec2(190.0, 30.0),
                                        )
                                    })
                                    .inner
                                    .on_hover_text(
                                        "Descarga la versión nueva, comprueba su firma, la instala \
                                         y vuelve a abrir el lanzador",
                                    )
                                    .on_disabled_hover_text(
                                        "Espera a que terminen las instalaciones en curso",
                                    );
                                if b.clicked() {
                                    self.start_self_update();
                                }
                            }
                            other => {
                                let why = match other {
                                    Some(Err(e)) => format!(
                                        "No puede actualizarse solo: {e}. Descárgalo desde GitHub."
                                    ),
                                    _ => "Descárgalo desde GitHub.".to_string(),
                                };
                                if flat_button(ui, "Descargar", Some(pal().text), vec2(110.0, 30.0))
                                    .on_hover_text(why)
                                    .clicked()
                                {
                                    super::open_link(&r.html_url);
                                }
                            }
                        }
                        if ui
                            .add(
                                egui::Label::new(caps("Novedades ↗", 10.5, pal().muted))
                                    .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            super::open_link(&r.html_url);
                        }
                    });
                });
                if let Some(super::SelfUpdate::Failed(e)) = &self.self_updating {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("No se pudo actualizar: {e}"))
                            .size(12.5)
                            .color(pal().error),
                    );
                }
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
            if !pending.is_empty()
                && flat_button(ui, &label, Some(pal().warn), vec2(0.0, 30.0)).clicked()
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
            caps_label(ui, &format!("{n:02} / {title}"), 11.0, pal().muted);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let s = if count == 1 { "app" } else { "apps" };
                caps_label(ui, &format!("{count} {s}"), 11.0, pal().faint);
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
            let cols = ((width / CELL_W).floor() as usize).max(1);
            let cell_w = (width / cols as f32).floor().min(CELL_W * 1.3);
            for row in items.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for item in row {
                        match item {
                            Some(i) => self.cell(ui, *i, sys, cell_w),
                            None => self.account_cell(ui, cell_w),
                        }
                    }
                });
            }
        }
        if n == 0 {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Ninguna app coincide con el filtro.").color(pal().muted));
            });
        }
    }

    /// Celda del mosaico: fondo al pasar el ratón, icono con su distintivo, nombre y una
    /// línea de texto. Devuelve la respuesta (clic = abrir la ficha).
    fn paint_cell(&self, ui: &mut Ui, w: f32, cell: Cell) -> egui::Response {
        let Cell {
            icon_id,
            accent,
            name,
            line,
            badge,
            dim,
            progress,
        } = cell;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, CELL_H), Sense::click());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let painter = ui.painter_at(rect);
        let hovered = resp.hovered();
        let bg = if hovered { pal().surface_hi } else { pal().bg };
        let card = rect.shrink(4.0);
        if hovered {
            painter.rect_filled(card, CornerRadius::same(10), pal().surface_hi);
        }
        if resp.has_focus() {
            painter.rect_stroke(
                card,
                CornerRadius::same(10),
                Stroke::new(1.5, accent),
                StrokeKind::Inside,
            );
        }
        let icon = Rect::from_center_size(
            pos2(rect.center().x, rect.top() + 18.0 + CELL_ICON / 2.0),
            vec2(CELL_ICON, CELL_ICON),
        );
        self.icons.paint(&painter, icon, icon_id, accent);
        if dim {
            painter.rect_filled(
                icon.expand(1.0),
                CornerRadius::same((CELL_ICON * 0.22) as u8),
                bg.gamma_multiply(0.62),
            );
        }
        if let Some(b) = badge {
            paint_badge(&painter, icon.right_bottom() - vec2(4.0, 4.0), 11.0, b, bg);
        }
        let text_w = w - 16.0;
        let mut name = name;
        name.wrap = egui::text::TextWrapping::truncate_at_width(text_w);
        let name = ui.fonts_mut(|f| f.layout_job(name));
        let y = icon.bottom() + 12.0;
        painter.galley(
            pos2(rect.center().x - name.size().x / 2.0, y),
            name,
            pal().text,
        );
        let y = y + 22.0;
        match progress {
            Some(p) => {
                let bar = Rect::from_center_size(pos2(rect.center().x, y + 7.0), vec2(72.0, 4.0));
                painter.rect_filled(bar, CornerRadius::same(2), pal().line);
                let mut done = bar;
                done.set_width(bar.width() * p.clamp(0.0, 1.0));
                painter.rect_filled(done, CornerRadius::same(2), accent);
            }
            None => {
                let mut job = LayoutJob::simple_singleline(line.0, theme::font(12.0), line.1);
                job.wrap = egui::text::TextWrapping::truncate_at_width(text_w);
                let g = ui.fonts_mut(|f| f.layout_job(job));
                painter.galley(pos2(rect.center().x - g.size().x / 2.0, y), g, line.1);
            }
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    fn cell(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo, w: f32) {
        let app: &'static AppDef = &APPS[i];
        let accent = theme::accent(app.accent);
        let status = self.status(i);
        let row = &self.rows[i];
        let progress = row.busy.as_ref().map(|_| row.progress.unwrap_or(0.0));
        let line = if row.error.is_some() {
            ("Error · ver detalles".to_string(), pal().error)
        } else {
            match &status {
                Status::UpdateAvailable => ("Actualización".into(), pal().warn),
                Status::NotInstalled => ("Disponible".into(), pal().info),
                Status::Unsupported => ("No disponible".into(), pal().faint),
                Status::Unavailable => ("Sin versión".into(), pal().faint),
                Status::Checking => ("Consultando…".into(), pal().faint),
                Status::UpToDate | Status::InstalledUnknown => (app.tagline.into(), pal().muted),
            }
        };
        let badge = match status {
            _ if progress.is_some() => None,
            Status::UpToDate | Status::InstalledUnknown => Some(Badge::Ok),
            Status::UpdateAvailable => Some(Badge::Update),
            Status::NotInstalled => Some(Badge::Get),
            Status::Unsupported | Status::Unavailable => Some(Badge::No),
            Status::Checking => None,
        };
        let dim = matches!(status, Status::Unsupported | Status::Unavailable);
        let cell = Cell {
            icon_id: app.id,
            accent,
            name: two_tone(app.name, 14.5, accent),
            line,
            badge,
            dim,
            progress,
        };
        let resp = self.paint_cell(ui, w, cell);
        let resp = resp.on_hover_text(format!("{} · {}", app.tagline, status_style(&status).1));
        if resp.clicked() {
            self.selected = Some(Selected::App(i));
        }
        resp.context_menu(|ui| {
            ui.set_min_width(190.0);
            if let Some((label, action)) = self.primary_action(i, sys) {
                if ui.button(label).clicked() {
                    self.run_action(i, action);
                }
            }
            if ui.button("Ver detalles").clicked() {
                self.selected = Some(Selected::App(i));
            }
            if ui.button("Novedades de la versión").clicked() {
                super::open_link(self.notes_url(i));
            }
            if self.rows[i].installed.is_some() && self.rows[i].busy.is_none() {
                ui.separator();
                if ui
                    .button(RichText::new("Desinstalar…").color(pal().error))
                    .clicked()
                {
                    self.confirm_uninstall = Some(i);
                }
            }
        });
    }

    fn account_cell(&mut self, ui: &mut Ui, w: f32) {
        let (color, text) = self.account_status();
        let badge = if self.account_connected() {
            Some(Badge::Ok)
        } else if self.account.saved.is_some() && self.account.loaded {
            Some(Badge::Warn)
        } else {
            None
        };
        let cell = Cell {
            icon_id: "iurefficient",
            accent: pal().info,
            name: two_tone("Iurefficient", 14.5, pal().info),
            line: (text.to_string(), color),
            badge,
            dim: false,
            progress: None,
        };
        let resp = self.paint_cell(ui, w, cell);
        let resp = resp.on_hover_text("Tu cuenta, para todas las apps");
        if resp.clicked() {
            self.selected = Some(Selected::Account);
        }
    }

    /// URL de las novedades de la última versión (o de la lista de versiones).
    pub(super) fn notes_url(&self, i: usize) -> String {
        self.rows[i]
            .release
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|r| r.html_url.clone())
            .unwrap_or_else(|| APPS[i].release_url())
    }

    /// Acción principal de la app según su estado, si hay alguna.
    pub(super) fn primary_action(
        &self,
        i: usize,
        sys: &SystemInfo,
    ) -> Option<(&'static str, Action)> {
        if self.rows[i].busy.is_some() {
            return None;
        }
        let has_release = matches!(self.rows[i].release, Some(Ok(_))) && APPS[i].supports(sys);
        let installed = self.rows[i].installed.is_some();
        match self.status(i) {
            Status::NotInstalled if has_release => Some(("Instalar", Action::Install)),
            Status::UpdateAvailable if has_release => Some(("Actualizar", Action::Update)),
            _ if installed && has_release && self.selection_differs(i, sys) => {
                Some(("Cambiar a la selección", Action::Install))
            }
            _ if installed => Some(("Abrir", Action::Open)),
            _ => None,
        }
    }

    pub(super) fn run_action(&mut self, i: usize, action: Action) {
        match action {
            Action::Open => self.launch(i),
            Action::Install | Action::Update => self.start_install(i),
        }
    }

    /// Estado de la cuenta: (color, etiqueta).
    pub(super) fn account_status(&self) -> (Color32, &'static str) {
        let a = &self.account;
        match &a.saved {
            _ if !a.loaded => (pal().muted, "Consultando…"),
            Some(_) if a.busy => (pal().muted, "Cerrando sesión…"),
            Some(_) if a.checking => (pal().muted, "Comprobando sesión…"),
            Some(s) if s.has_session && !a.expired => (pal().ok, "Conectada"),
            Some(_) => (pal().warn, "Sesión caducada"),
            None => (pal().info, "Sin conectar"),
        }
    }

    pub(super) fn account_connected(&self) -> bool {
        self.account.saved.as_ref().is_some_and(|s| s.has_session) && !self.account.expired
    }

    fn account_actions(&mut self, ui: &mut Ui, width: f32) {
        let size = vec2(width - 44.0, 36.0);
        let busy = !self.account.loaded || self.account.busy || self.account.checking;
        let saved = self.account.saved.clone();
        if self.account_connected() {
            if let Some(s) = &saved {
                if flat_button(ui, "Abrir Iurefficient", Some(pal().text), size)
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
                .add_enabled_ui(!busy, |ui| flat_button(ui, label, Some(pal().text), size))
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
            let menu = egui::Button::new(RichText::new("···").size(15.0).color(pal().text))
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
                        .button(RichText::new("Cerrar sesión…").color(pal().error))
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
                    .color(pal().muted),
            );
            ui.add_space(12.0);
            let field = |ui: &mut Ui, label: &str, value: &mut String, hint: &str, password: bool, enabled: bool| {
                caps_label(ui, label, 10.0, pal().faint);
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
                ui.label(RichText::new(e).size(12.5).color(pal().error));
                ui.add_space(6.0);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ready = !f.busy
                    && !f.domain.trim().is_empty()
                    && !f.email.trim().is_empty()
                    && (!f.password.is_empty() || f.totp_token.is_some());
                let label = if f.totp_token.is_some() { "Verificar" } else { "Conectar" };
                if ui.add_enabled_ui(ready, |ui| flat_button(ui, label, Some(pal().text), vec2(130.0, 32.0))).inner.clicked() {
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
                    .color(pal().muted),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if flat_button(ui, "Cerrar sesión", Some(pal().error), vec2(140.0, 32.0)).clicked() {
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

    /// Variante (IureTranscribe), formato y avisos, en una línea fija del mosaico.
    pub(super) fn tile_options(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo) {
        let app: &'static AppDef = &APPS[i];
        if let Some(e) = self.rows[i].error.clone() {
            ui.add(egui::Label::new(RichText::new(&e).size(12.0).color(pal().error)).truncate())
                .on_hover_text(e);
            return;
        }
        let Some(Ok(release)) = self.rows[i].release.clone() else {
            if !app.note.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(app.note).size(12.0).color(pal().faint))
                        .truncate(),
                );
            }
            return;
        };
        if !app.supports(sys) {
            if !app.note.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(app.note).size(12.0).color(pal().faint))
                        .truncate(),
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
        ui.horizontal(|ui| {
            if app.gpu_variants {
                let (recommended, why) = app.recommend_variant(sys);
                caps_label(ui, "Aceleración", 10.0, pal().faint);
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
                        caps_label(ui, "Formato", 10.0, pal().faint);
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
                    ui.label(RichText::new(cur.label()).size(12.0).color(pal().faint));
                } else if !app.note.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(app.note).size(12.0).color(pal().faint))
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
                    caps_label(ui, &format!("{text} {:.0}%", p * 100.0), 10.0, pal().muted);
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
                if flat_button(ui, "Instalar", Some(pal().text), size).clicked() {
                    self.start_install(i);
                }
            }
            Status::UpdateAvailable if has_release => {
                if flat_button(ui, "Actualizar", Some(pal().warn), size).clicked() {
                    self.start_install(i);
                }
            }
            _ if installed && differs && has_release => {
                if flat_button(ui, "Cambiar a la selección", Some(pal().text), size).clicked() {
                    self.start_install(i);
                }
            }
            _ if installed => {
                if flat_button(ui, "Abrir", Some(pal().text), size).clicked() {
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
            let menu = egui::Button::new(RichText::new("···").size(15.0).color(pal().text))
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
                        .button(RichText::new("Desinstalar…").color(pal().error))
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
            .stroke(Stroke::new(1.0, pal().line))
            .fill(pal().surface)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::Grid::new("apps-table")
                    .num_columns(6)
                    .spacing([18.0, 10.0])
                    .striped(false)
                    .show(ui, |ui| {
                        for h in ["", "App", "Estado", "Instalada", "Última", ""] {
                            caps_label(ui, h, 10.0, pal().faint);
                        }
                        ui.end_row();
                        if self.account_matches() {
                            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
                            self.icons
                                .paint(ui.painter(), r, "iurefficient", pal().info);
                            ui.vertical(|ui| {
                                ui.set_min_width(230.0);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                let logo = self.icons.logo(ui, 14.0).interact(Sense::click());
                                if logo
                                    .on_hover_text("Ver detalles")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.selected = Some(Selected::Account);
                                }
                                ui.add(
                                    egui::Label::new(
                                        RichText::new("Cuenta para todas las apps")
                                            .size(12.0)
                                            .color(pal().faint),
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
                            ui.label(RichText::new(who).color(pal().muted));
                            let dom = self
                                .account
                                .saved
                                .as_ref()
                                .map(|s| s.domain.clone())
                                .unwrap_or_else(|| "—".into());
                            ui.label(RichText::new(dom).color(pal().muted));
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
                                let name = egui::Label::new(two_tone(app.name, 15.0, accent))
                                    .extend()
                                    .sense(Sense::click());
                                if ui
                                    .add(name)
                                    .on_hover_text("Ver detalles")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.selected = Some(Selected::App(i));
                                }
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(app.tagline).size(12.0).color(pal().faint),
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
                            ui.label(RichText::new(inst).color(pal().muted));
                            let latest = match &self.rows[i].release {
                                Some(Ok(r)) => format!("{} · {}", r.version(), nice_date(r.date())),
                                Some(Err(_)) => "—".into(),
                                None => "…".into(),
                            };
                            ui.label(RichText::new(latest).color(pal().muted));
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
            caps_label(ui, &format!("{checked}{source}"), 10.0, pal().faint);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = format!("Registro ({})", self.log.len());
                if ui
                    .add(egui::Label::new(caps(&label, 10.0, pal().muted)).sense(Sense::click()))
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
                    caps_label(
                        ui,
                        &format!("Instalando para {os} {arch}"),
                        10.0,
                        pal().faint,
                    );
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
                            caps_label(ui, k, 10.0, pal().faint);
                            ui.label(RichText::new(v).color(pal().text));
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
                            ui.label(RichText::new("Sin actividad todavía.").color(pal().faint));
                        }
                        for l in &self.log {
                            ui.label(
                                RichText::new(l)
                                    .font(FontId::monospace(11.5))
                                    .color(pal().muted),
                            );
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
                    .color(pal().muted),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if flat_button(ui, "Desinstalar", Some(pal().error), vec2(130.0, 32.0)).clicked() {
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
        self.sync_theme();
        let ctx = ui.ctx().clone();
        // Para que «Consultado hace…» avance sin interacción.
        ctx.request_repaint_after(std::time::Duration::from_secs(30));

        let bar = egui::Frame::new()
            .fill(pal().bg)
            .inner_margin(egui::Margin::symmetric(28, 6));
        egui::Panel::top("header")
            .frame(bar)
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("footer")
            .frame(bar)
            .show(ui, |ui| self.footer(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal().bg)
                    .inner_margin(egui::Margin {
                        left: 28,
                        right: 28,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show(ui, |ui| {
                // Líneas de separación de cabecera y pie, de borde a borde.
                let full = ui.max_rect().expand2(vec2(28.0, 0.0));
                ui.painter()
                    .hline(full.x_range(), full.top(), Stroke::new(1.0, pal().line));
                ui.painter()
                    .hline(full.x_range(), full.bottom(), Stroke::new(1.0, pal().line));
                // Cada ficha con su propio desplazamiento: se abre arriba y, al volver, el
                // mosaico sigue donde estaba.
                let scroll_id = match self.selected {
                    None => "list".to_string(),
                    Some(Selected::Account) => "detail-account".to_string(),
                    Some(Selected::App(i)) => format!("detail-{i}"),
                };
                egui::ScrollArea::vertical()
                    .id_salt(scroll_id)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Hueco para la barra de desplazamiento.
                        ui.set_max_width(ui.available_width() - 14.0);
                        self.update_banner(ui);
                        match self.sys.clone() {
                            None => {
                                self.filter_bar(ui);
                                ui.add_space(40.0);
                                ui.vertical_centered(|ui| {
                                    ui.spinner();
                                    ui.label(
                                        RichText::new("Analizando el equipo…").color(pal().muted),
                                    );
                                });
                            }
                            Some(sys) if self.selected.is_some() => self.detail(ui, &sys),
                            Some(sys) => {
                                self.filter_bar(ui);
                                match self.view {
                                    ViewMode::Grid => self.grid(ui, &sys),
                                    ViewMode::Table => self.table(ui, &sys),
                                }
                            }
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
        // Esc (o el botón «atrás» del ratón) cierra la ficha si no hay un diálogo encima.
        let modal = self.viewer.is_some()
            || self.confirm_uninstall.is_some()
            || self.login.is_some()
            || self.confirm_logout;
        if self.selected.is_some()
            && !modal
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.pointer.button_pressed(egui::PointerButton::Extra1)
            })
        {
            self.selected = None;
        }
        self.uninstall_modal(&ctx);
        self.login_modal(&ctx);
        self.logout_modal(&ctx);
        self.shot_viewer(&ctx);
    }
}
