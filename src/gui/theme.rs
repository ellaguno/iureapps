//! Aspecto del lanzador: paleta, fuentes incrustadas e iconos de las apps.

use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Painter, Rect, Shape, Stroke, TextureHandle, TextureOptions,
};
use std::collections::HashMap;
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(12, 14, 19);
pub const SURFACE: Color32 = Color32::from_rgb(17, 20, 27);
pub const SURFACE_HI: Color32 = Color32::from_rgb(24, 28, 37);
pub const LINE: Color32 = Color32::from_rgb(38, 43, 54);
pub const TEXT: Color32 = Color32::from_rgb(232, 234, 239);
pub const MUTED: Color32 = Color32::from_rgb(140, 147, 160);
pub const FAINT: Color32 = Color32::from_rgb(92, 99, 112);
pub const BRAND: Color32 = Color32::from_rgb(6, 80, 236);
pub const BRAND_NAVY: Color32 = Color32::from_rgb(17, 40, 76);
pub const OK: Color32 = Color32::from_rgb(52, 199, 120);
pub const WARN: Color32 = Color32::from_rgb(245, 166, 35);
pub const INFO: Color32 = Color32::from_rgb(96, 150, 255);
pub const ERROR: Color32 = Color32::from_rgb(240, 90, 90);

pub fn accent(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Familia de la fuente en negrita (Inter Bold).
pub fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, bold())
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(
        "inter",
        include_bytes!("../../assets/fonts/Inter-Regular.otf"),
    );
    add(
        "inter-bold",
        include_bytes!("../../assets/fonts/Inter-Bold.otf"),
    );
    add(
        "jbmono",
        include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf"),
    );
    // Las fuentes de egui quedan detrás como respaldo para cualquier signo que falte.
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let with = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, with("inter"));
    fonts.families.insert(bold(), with("inter-bold"));
    fonts.families.insert(FontFamily::Monospace, with("jbmono"));
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.window_stroke = Stroke::new(1.0, LINE);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.extreme_bg_color = Color32::from_rgb(9, 11, 15);
    v.faint_bg_color = SURFACE;
    v.hyperlink_color = TEXT;
    v.selection.bg_fill = BRAND;
    v.override_text_color = None;
    let r = CornerRadius::same(2);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    v.widgets.inactive.weak_bg_fill = SURFACE_HI;
    v.widgets.inactive.bg_fill = SURFACE_HI;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(32, 37, 48);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(70, 78, 94));
    v.widgets.active.weak_bg_fill = Color32::from_rgb(40, 46, 60);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|s| {
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.text_styles.insert(egui::TextStyle::Body, font(14.0));
        s.text_styles.insert(egui::TextStyle::Button, font(13.5));
        s.text_styles.insert(egui::TextStyle::Small, font(11.5));
        s.text_styles.insert(egui::TextStyle::Monospace, mono(12.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, bold_font(20.0));
    });
}

/// Iconos de las apps que publican uno; el resto se dibuja con [`paint_drawn_icon`].
pub struct Icons {
    textures: HashMap<&'static str, TextureHandle>,
}

impl Icons {
    pub fn load(ctx: &egui::Context) -> Self {
        let files: [(&'static str, &'static [u8]); 4] = [
            (
                "iureditor",
                include_bytes!("../../assets/icons/iureditor.png"),
            ),
            (
                "iuretranscribe",
                include_bytes!("../../assets/icons/iuretranscribe.png"),
            ),
            ("iureocr", include_bytes!("../../assets/icons/iureocr.png")),
            ("iuredav", include_bytes!("../../assets/icons/iuredav.png")),
        ];
        let textures = files
            .into_iter()
            .filter_map(|(id, bytes)| {
                let img = decode_png(bytes)?;
                Some((id, ctx.load_texture(id, img, TextureOptions::LINEAR)))
            })
            .collect();
        Icons { textures }
    }

    pub fn paint(&self, painter: &Painter, rect: Rect, app_id: &str, accent: Color32) {
        match self.textures.get(app_id) {
            Some(t) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(t.id(), rect, uv, Color32::WHITE);
            }
            None => paint_drawn_icon(painter, rect, app_id, accent),
        }
    }
}

/// Icono de la ventana (el mismo que instalan los paquetes .deb y .rpm).
pub fn window_icon() -> Option<egui::IconData> {
    let img = decode_png(include_bytes!("../../assets/iureapps.png"))?;
    Some(egui::IconData {
        rgba: img
            .pixels
            .iter()
            .flat_map(|c| c.to_srgba_unmultiplied())
            .collect(),
        width: img.size[0] as u32,
        height: img.size[1] as u32,
    })
}

fn decode_png(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    let size = [info.width as usize, info.height as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, &rgba))
}

/// Icono al estilo de la suite (fondo azul marino y el semicírculo de Iurefficient) con un
/// símbolo propio, para las apps que todavía no publican icono.
pub fn paint_drawn_icon(painter: &Painter, rect: Rect, app_id: &str, accent: Color32) {
    let s = rect.width();
    painter.rect_filled(rect, CornerRadius::same((s * 0.22) as u8), BRAND_NAVY);
    // Semicírculo superior (la «cúpula» del logotipo).
    let c = pos2(rect.center().x, rect.top() + s * 0.12);
    let r = s * 0.26;
    let points: Vec<_> = (0..=24)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / 24.0;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(Shape::convex_polygon(points, BRAND, Stroke::NONE));

    let white = Color32::from_rgb(246, 246, 248);
    let base = rect.top() + s * 0.52;
    match app_id {
        "iurevoz" => {
            // Onda de voz: barras verticales redondeadas.
            let heights = [0.10, 0.22, 0.34, 0.22, 0.10];
            let w = s * 0.07;
            for (k, h) in heights.iter().enumerate() {
                let x = rect.center().x + (k as f32 - 2.0) * s * 0.12;
                let half = s * h / 2.0;
                let bar = Rect::from_center_size(pos2(x, base + s * 0.17), vec2(w, half * 2.0));
                let color = if k == 2 { accent } else { white };
                painter.rect_filled(bar, CornerRadius::same((w / 2.0) as u8), color);
            }
        }
        "iurefficient" => {
            // Cuenta: silueta de persona con un anillo del color de acento.
            let head = pos2(rect.center().x, base + s * 0.08);
            painter.circle_filled(head, s * 0.085, white);
            let body = Rect::from_center_size(
                pos2(rect.center().x, base + s * 0.29),
                vec2(s * 0.36, s * 0.17),
            );
            painter.rect_filled(
                body,
                CornerRadius {
                    nw: (s * 0.12) as u8,
                    ne: (s * 0.12) as u8,
                    sw: 2,
                    se: 2,
                },
                white,
            );
            painter.circle_stroke(
                pos2(rect.center().x, base + s * 0.17),
                s * 0.27,
                Stroke::new(s * 0.035, accent),
            );
        }
        _ => {
            // Diagrama de Gantt: tres barras escalonadas.
            let rows = [(0.18, 0.40), (0.34, 0.36), (0.52, 0.30)];
            let h = s * 0.085;
            for (k, (x0, w)) in rows.iter().enumerate() {
                let y = base + k as f32 * s * 0.13;
                let bar = Rect::from_min_size(pos2(rect.left() + s * x0, y), vec2(s * w, h));
                let color = if k == 1 { accent } else { white };
                painter.rect_filled(bar, CornerRadius::same((h / 2.0) as u8), color);
            }
        }
    }
}
