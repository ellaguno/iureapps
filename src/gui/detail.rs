//! Ficha de una app (o de la cuenta de Iurefficient), en lugar del mosaico: portada con el
//! color de la app, acciones, variante y formato, capturas, datos técnicos y novedades.

use super::theme::{self, pal};
use super::view::{caps, caps_label, nice_date, press_button, status_style, two_tone, Action};
use super::{LauncherApp, Selected, Shots};
use crate::catalog::{AppDef, APPS};
use crate::plan::{self, Status};
use crate::system::SystemInfo;
use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui,
    UiBuilder,
};

const HERO_H: f32 = 150.0;
const HERO_ICON: f32 = 104.0;
/// La portada y el texto no pasan de este ancho en ventanas muy anchas.
const MAX_W: f32 = 1040.0;
const SHOT_H: f32 = 230.0;

/// Estilo de los botones de la ficha; el color dice qué hace, igual que el distintivo
/// del mosaico.
#[derive(Clone, Copy)]
enum Kind {
    Open,
    Install,
    Update,
    Danger,
    Plain,
}

fn button(ui: &mut Ui, text: &str, kind: Kind) -> egui::Response {
    let (fill, fg, stroke) = match kind {
        Kind::Open => (pal().ok, pal().bg, Stroke::NONE),
        Kind::Install => (pal().info, pal().bg, Stroke::NONE),
        Kind::Update => (pal().warn, pal().bg, Stroke::NONE),
        Kind::Danger => (
            Color32::TRANSPARENT,
            pal().error,
            Stroke::new(1.0, pal().error),
        ),
        Kind::Plain => (
            Color32::TRANSPARENT,
            pal().text,
            Stroke::new(1.0, pal().line),
        ),
    };
    let min_w = if matches!(kind, Kind::Open | Kind::Install | Kind::Update) {
        140.0
    } else {
        0.0
    };
    press_button(
        ui,
        caps(text, 11.5, fg),
        fill,
        stroke,
        CornerRadius::same(4),
        vec2(min_w, 34.0),
    )
}

fn link(ui: &mut Ui, text: &str, url: &str) {
    if ui
        .add(egui::Label::new(caps(text, 10.5, pal().muted)).sense(Sense::click()))
        .on_hover_text(url)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        super::open_link(url);
    }
}

/// Degradado horizontal (de `a` a `b`), algo más oscuro abajo a la izquierda.
fn gradient(painter: &egui::Painter, rect: Rect, a: Color32, b: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(rect.left_top(), a);
    m.colored_vertex(rect.right_top(), b);
    m.colored_vertex(rect.left_bottom(), a.lerp_to_gamma(b, 0.55));
    m.colored_vertex(rect.right_bottom(), b);
    m.add_triangle(0, 1, 2);
    m.add_triangle(1, 3, 2);
    painter.add(m);
}

/// Dato de la ficha: etiqueta pequeña y valor.
fn fact(ui: &mut Ui, label: &str, value: &str, color: Color32) {
    ui.allocate_ui_with_layout(
        vec2(190.0, 44.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(190.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            caps_label(ui, label, 10.0, pal().faint);
            ui.add(egui::Label::new(RichText::new(value).size(13.5).color(color)).truncate())
                .on_hover_text(value);
        },
    );
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(26.0);
    caps_label(ui, title, 11.0, pal().muted);
    ui.add_space(10.0);
}

impl LauncherApp {
    /// Ficha de lo seleccionado, con «‹ Todas las apps» para volver al mosaico.
    pub(super) fn detail(&mut self, ui: &mut Ui, sys: &SystemInfo) {
        let Some(sel) = self.selected else { return };
        ui.add_space(16.0);
        if ui
            .add(
                egui::Label::new(caps("‹  Todas las apps", 11.0, pal().muted))
                    .sense(Sense::click()),
            )
            .on_hover_text("Esc")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.selected = None;
            return;
        }
        ui.add_space(12.0);
        ui.set_max_width(ui.available_width().min(MAX_W));
        match sel {
            Selected::Account => self.account_detail(ui),
            Selected::App(i) => self.app_detail(ui, i, sys),
        }
    }

    /// Portada a todo lo ancho con el icono montado sobre su borde inferior, y a la derecha
    /// del icono lo que pinte `title`.
    fn hero(&self, ui: &mut Ui, icon_id: &str, accent: Color32, title: impl FnOnce(&mut Ui)) {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, HERO_H), Sense::hover());
        let painter = ui.painter_at(rect);
        gradient(
            &painter,
            rect,
            accent.lerp_to_gamma(pal().brand_navy, 0.3),
            pal().brand_navy.lerp_to_gamma(pal().bg, 0.45),
        );
        // El icono en grande y tenue, como marca de agua.
        if let Some(t) = self.icons.texture(icon_id) {
            let big = Rect::from_center_size(
                pos2(rect.right() - 130.0, rect.center().y + 24.0),
                vec2(280.0, 280.0),
            );
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(t.id(), big, uv, Color32::from_white_alpha(26));
        }
        let icon = Rect::from_min_size(
            pos2(rect.left() + 28.0, rect.bottom() - HERO_ICON + 44.0),
            vec2(HERO_ICON, HERO_ICON),
        );
        let p = ui.painter();
        p.rect_filled(
            icon.expand(5.0),
            CornerRadius::same(((HERO_ICON + 10.0) * 0.22) as u8),
            pal().bg,
        );
        self.icons.paint(p, icon, icon_id, accent);

        let area = Rect::from_min_max(
            pos2(icon.right() + 20.0, rect.bottom() + 10.0),
            pos2(rect.right(), rect.bottom() + 80.0),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(area)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                title(ui);
            },
        );
        let below = icon.bottom() + 20.0;
        let y = ui.cursor().top();
        if y < below {
            ui.add_space(below - y);
        }
    }

    fn app_detail(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo) {
        let app: &'static AppDef = &APPS[i];
        let accent = theme::accent(app.accent);
        self.hero(ui, app.id, accent, |ui| {
            ui.add(egui::Label::new(two_tone(app.name, 30.0, accent)).truncate());
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{} · {}", app.tagline, app.hint))
                        .size(14.0)
                        .color(pal().muted),
                )
                .truncate(),
            );
        });

        self.app_actions(ui, i, sys);
        ui.add_space(12.0);
        if self.rows[i].busy.is_none() {
            self.tile_options(ui, i, sys);
        }
        if self.status(i) == Status::Unsupported {
            ui.label(
                RichText::new(format!(
                    "{} todavía no tiene versión para {}.",
                    app.name,
                    sys.platform_label()
                ))
                .color(pal().warn),
            );
        }

        section(ui, "Capturas");
        self.shots_strip(ui, i);

        section(ui, "Detalles");
        self.app_facts(ui, i, sys);

        section(ui, "Acerca de");
        ui.label(RichText::new(app.description).size(15.0).color(pal().text));
        let release = self.rows[i].release.as_ref().and_then(|r| r.as_ref().ok());
        if let Some(r) = release {
            let notes = r.body.as_deref().unwrap_or("").trim();
            if !notes.is_empty() {
                section(ui, &format!("Novedades de la versión {}", r.version()));
                egui::Frame::new()
                    .fill(pal().surface)
                    .stroke(Stroke::new(1.0, pal().line))
                    .inner_margin(14)
                    .show(ui, |ui| {
                        // Completas: la ficha ya tiene su propio desplazamiento.
                        ui.set_width(ui.available_width());
                        super::markdown::show(ui, notes);
                    });
            }
        }
    }

    fn app_actions(&mut self, ui: &mut Ui, i: usize, sys: &SystemInfo) {
        let app: &'static AppDef = &APPS[i];
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            if let Some(text) = self.rows[i].busy.clone() {
                let p = self.rows[i].progress.unwrap_or(0.0);
                ui.add(
                    egui::ProgressBar::new(p)
                        .desired_width(280.0)
                        .desired_height(8.0)
                        .fill(theme::accent(app.accent))
                        .corner_radius(0.0),
                );
                ui.spinner();
                caps_label(ui, &format!("{text} {:.0}%", p * 100.0), 10.5, pal().muted);
                return;
            }
            let installed = self.rows[i].installed.is_some();
            match self.primary_action(i, sys) {
                Some((label, action)) => {
                    let kind = match action {
                        Action::Open => Kind::Open,
                        Action::Install => Kind::Install,
                        Action::Update => Kind::Update,
                    };
                    if button(ui, label, kind).clicked() {
                        self.run_action(i, action);
                    }
                    // Con una actualización o un cambio pendiente, abrir sigue a mano.
                    if installed
                        && !matches!(action, Action::Open)
                        && button(ui, "Abrir", Kind::Plain).clicked()
                    {
                        self.launch(i);
                    }
                }
                None => {
                    ui.add_enabled_ui(false, |ui| button(ui, "No disponible", Kind::Plain));
                }
            }
            let has_release = matches!(self.rows[i].release, Some(Ok(_))) && app.supports(sys);
            if installed
                && has_release
                && self.status(i) == Status::UpToDate
                && button(ui, "Reinstalar", Kind::Plain)
                    .on_hover_text("Volver a instalar la misma versión")
                    .clicked()
            {
                self.start_install(i);
            }
            if installed && button(ui, "Desinstalar…", Kind::Danger).clicked() {
                self.confirm_uninstall = Some(i);
            }
            ui.add_space(10.0);
            link(ui, "Novedades ↗", &self.notes_url(i));
            ui.add_space(6.0);
            link(ui, "Versiones ↗", &app.release_url());
            ui.add_space(6.0);
            link(ui, "GitHub ↗", &format!("https://github.com/{}", app.repo));
        });
        if let Some(e) = &self.rows[i].error {
            ui.add_space(6.0);
            ui.label(RichText::new(e).size(13.0).color(pal().error));
        }
    }

    fn app_facts(&self, ui: &mut Ui, i: usize, sys: &SystemInfo) {
        let app: &'static AppDef = &APPS[i];
        let row = &self.rows[i];
        let status = self.status(i);
        let release = row.release.as_ref().and_then(|r| r.as_ref().ok());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(18.0, 14.0);
            let (color, text) = status_style(&status);
            fact(ui, "Estado", text, color);
            let installed = match &row.installed {
                Some(inst) => {
                    let mut s = format!(
                        "{} · {}",
                        inst.version.as_deref().unwrap_or("?"),
                        inst.format.label()
                    );
                    if let Some(v) = inst.variant.filter(|_| app.gpu_variants) {
                        s.push_str(&format!(" · {}", v.label()));
                    }
                    s
                }
                None => "—".into(),
            };
            fact(ui, "Instalada", &installed, pal().text);
            let latest = match &row.release {
                Some(Ok(r)) => format!("{} · {}", r.version(), nice_date(r.date())),
                Some(Err(e)) => e.clone(),
                None => "Consultando…".into(),
            };
            fact(ui, "Última versión", &latest, pal().text);
            if let Some(r) = release.filter(|_| app.supports(sys)) {
                let pref = self.state.prefs.get(app.id);
                let v = plan::variant(app, sys, row.installed.as_ref(), pref);
                if let Some(f) = plan::format(app, sys, r, v, row.installed.as_ref(), pref) {
                    let size = app
                        .select_asset(r, sys, v, f)
                        .map(|(a, _)| a.size)
                        .filter(|s| *s > 0)
                        .map(|s| format!(" · {}", crate::util::human_size(s)))
                        .unwrap_or_default();
                    fact(
                        ui,
                        "Instalador",
                        &format!("{}{size}", f.label()),
                        pal().text,
                    );
                }
                if app.gpu_variants {
                    let (rec, _) = app.recommend_variant(sys);
                    fact(ui, "Aceleración recomendada", rec.label(), pal().text);
                }
            }
            fact(ui, "Repositorio", app.repo, pal().muted);
        });
    }

    /// Capturas en fila (con desplazamiento horizontal); un clic las amplía.
    fn shots_strip(&mut self, ui: &mut Ui, i: usize) {
        self.load_shots(i);
        let mut open = None;
        match self.shots.get(&i) {
            Some(Shots::Ready(v)) if !v.is_empty() => {
                egui::ScrollArea::horizontal()
                    .id_salt(("shots", i))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 14.0;
                            for (k, (caption, tex)) in v.iter().enumerate() {
                                let s = tex.size_vec2();
                                let mut size = vec2(SHOT_H * s.x / s.y, SHOT_H);
                                if size.x > 520.0 {
                                    size = vec2(520.0, 520.0 * s.y / s.x);
                                }
                                ui.vertical(|ui| {
                                    ui.set_width(size.x);
                                    let r = ui
                                        .add(
                                            egui::Image::new((tex.id(), size))
                                                .corner_radius(4)
                                                .sense(Sense::click()),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::ZoomIn);
                                    let line = if r.hovered() { pal().muted } else { pal().line };
                                    ui.painter().rect_stroke(
                                        r.rect,
                                        CornerRadius::same(4),
                                        Stroke::new(1.0, line),
                                        StrokeKind::Outside,
                                    );
                                    if r.clicked() {
                                        open = Some(k);
                                    }
                                    if !caption.is_empty() {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(caption)
                                                    .size(12.0)
                                                    .color(pal().faint),
                                            )
                                            .truncate(),
                                        );
                                    }
                                });
                            }
                        });
                        ui.add_space(6.0);
                    });
            }
            Some(Shots::Ready(_)) => {
                ui.label(
                    RichText::new("Esta app todavía no publica capturas.")
                        .size(13.0)
                        .color(pal().faint),
                )
                .on_hover_text(format!(
                    "Se toman de {}/1.png, 2.png… en github.com/{}",
                    crate::shots::DIR,
                    APPS[i].repo
                ));
            }
            Some(Shots::Failed(e)) => {
                ui.label(
                    RichText::new(format!("No se pudieron cargar las capturas: {e}"))
                        .size(13.0)
                        .color(pal().faint),
                );
            }
            Some(Shots::Loading) | None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("Cargando capturas…")
                            .size(13.0)
                            .color(pal().faint),
                    );
                });
            }
        }
        if let Some(k) = open {
            self.viewer = Some((i, k));
        }
    }

    /// Captura ampliada, con flechas para pasar a la anterior o la siguiente.
    pub(super) fn shot_viewer(&mut self, ctx: &egui::Context) {
        let Some((i, k)) = self.viewer else { return };
        let Some(Shots::Ready(v)) = self.shots.get(&i) else {
            self.viewer = None;
            return;
        };
        let n = v.len();
        let Some((caption, tex)) = v.get(k).cloned() else {
            self.viewer = None;
            return;
        };
        let screen = ctx.content_rect().size();
        let max = vec2(screen.x * 0.86, screen.y * 0.86 - 70.0).max(vec2(200.0, 120.0));
        let s = tex.size_vec2();
        let scale = (max.x / s.x).min(max.y / s.y).min(1.5);
        let size = s * scale;
        let mut go: i32 = 0;
        let mut close = false;
        let resp = egui::Modal::new(egui::Id::unique("iureapps-shot")).show(ctx, |ui| {
            ui.add(egui::Image::new((tex.id(), size)).corner_radius(4));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.set_width(size.x);
                let label = if caption.is_empty() {
                    format!("{} · {} de {n}", APPS[i].name, k + 1)
                } else {
                    format!("{caption} · {} de {n}", k + 1)
                };
                ui.label(RichText::new(label).color(pal().muted));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if button(ui, "Cerrar", Kind::Plain).clicked() {
                        close = true;
                    }
                    if n > 1 {
                        if button(ui, "›", Kind::Plain).clicked() {
                            go = 1;
                        }
                        if button(ui, "‹", Kind::Plain).clicked() {
                            go = -1;
                        }
                    }
                });
            });
        });
        ctx.input(|inp| {
            if inp.key_pressed(egui::Key::ArrowRight) {
                go = 1;
            }
            if inp.key_pressed(egui::Key::ArrowLeft) {
                go = -1;
            }
        });
        if close || resp.should_close() {
            self.viewer = None;
        } else if go != 0 && n > 0 {
            let k = (k as i32 + go).rem_euclid(n as i32) as usize;
            self.viewer = Some((i, k));
        }
    }

    fn account_detail(&mut self, ui: &mut Ui) {
        let connected = self.account_connected();
        let saved = self.account.saved.clone();
        let (color, status) = self.account_status();
        let tagline = match (&saved, connected) {
            (Some(s), true) => format!(
                "Conectada como {}",
                self.account.name.clone().unwrap_or_else(|| s.email.clone())
            ),
            (Some(_), false) => "Vuelve a iniciar sesión".to_string(),
            (None, _) => "Tu cuenta, para todas las apps".to_string(),
        };
        self.hero(ui, "iurefficient", pal().info, |ui| {
            ui.add_space(4.0);
            self.icons.logo(ui, 28.0);
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(RichText::new(tagline).size(14.0).color(pal().muted)).truncate(),
            );
        });

        let busy = !self.account.loaded || self.account.busy || self.account.checking;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            match &saved {
                Some(s) if connected => {
                    if button(ui, "Abrir Iurefficient", Kind::Open)
                        .on_hover_text(&s.web_url)
                        .clicked()
                    {
                        super::open_link(&s.web_url);
                    }
                }
                _ => {
                    let label = if saved.is_some() {
                        "Volver a conectar"
                    } else {
                        "Conectar"
                    };
                    let r = ui.add_enabled_ui(!busy, |ui| button(ui, label, Kind::Install));
                    if r.inner.clicked() {
                        self.open_login();
                    }
                }
            }
            if saved.is_some() {
                if connected && button(ui, "Cambiar de cuenta…", Kind::Plain).clicked() {
                    self.open_login();
                }
                if !busy && button(ui, "Cerrar sesión…", Kind::Danger).clicked() {
                    self.confirm_logout = true;
                }
            }
            if busy {
                ui.spinner();
            }
            ui.add_space(10.0);
            if let Some(s) = &saved {
                link(ui, "Instancia ↗", &s.web_url);
                ui.add_space(6.0);
            }
            link(
                ui,
                "Conector ↗",
                "https://github.com/ellaguno/iurefficient-connect",
            );
        });
        if let Some(e) = &self.account.error {
            ui.add_space(6.0);
            ui.label(RichText::new(e).size(13.0).color(pal().error));
        }

        section(ui, "Detalles");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(18.0, 14.0);
            fact(ui, "Estado", status, color);
            if let Some(s) = &saved {
                if let Some(name) = &self.account.name {
                    fact(ui, "Usuario", name, pal().text);
                }
                fact(ui, "Correo", &s.email, pal().text);
                fact(ui, "Instancia", &s.domain, pal().text);
                let dav = if s.has_webdav {
                    "Contraseña de aplicación guardada"
                } else {
                    "Sin contraseña de aplicación"
                };
                fact(ui, "WebDAV", dav, pal().text);
                if !s.app.is_empty() {
                    fact(ui, "Configurada por", &s.app, pal().text);
                }
            }
        });

        section(ui, "Acerca de");
        ui.label(
            RichText::new(
                "Inicia sesión una vez y IureDav, IureTranscribe, IureOCR e iureditor toman \
                 esta cuenta al abrirse, si no tienen otra configurada. La sesión se guarda en \
                 el llavero del sistema y, si hace falta, se crea la contraseña de aplicación \
                 WebDAV de este equipo.",
            )
            .size(15.0)
            .color(pal().text),
        );
    }
}
