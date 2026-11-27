//! Aspecto del lanzador: paleta, fuentes incrustadas e iconos de las apps.

use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId,
    Painter, Rect, Shape, Stroke, TextureHandle, TextureOptions,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Colores de la interfaz para un tema (claro u oscuro).
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub brand: Color32,
    pub brand_navy: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub info: Color32,
    pub error: Color32,
    /// Fondo del aviso de versión nueva.
    pub banner: Color32,
    /// Campos de texto.
    pub field: Color32,
    pub hover: Color32,
    pub hover_line: Color32,
    pub active: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(12, 14, 19),
    surface: Color32::from_rgb(17, 20, 27),
    surface_hi: Color32::from_rgb(24, 28, 37),
    line: Color32::from_rgb(38, 43, 54),
    text: Color32::from_rgb(232, 234, 239),
    muted: Color32::from_rgb(140, 147, 160),
    faint: Color32::from_rgb(92, 99, 112),
    brand: Color32::from_rgb(6, 80, 236),
    brand_navy: Color32::from_rgb(17, 40, 76),
    ok: Color32::from_rgb(52, 199, 120),
    warn: Color32::from_rgb(245, 166, 35),
    info: Color32::from_rgb(96, 150, 255),
    error: Color32::from_rgb(240, 90, 90),
    banner: Color32::from_rgb(14, 22, 40),
    field: Color32::from_rgb(9, 11, 15),
    hover: Color32::from_rgb(32, 37, 48),
    hover_line: Color32::from_rgb(70, 78, 94),
    active: Color32::from_rgb(40, 46, 60),
};

/// Tema claro: los colores de estado, más oscuros, para que se lean sobre blanco.
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(246, 247, 249),
    surface: Color32::from_rgb(255, 255, 255),
    surface_hi: Color32::from_rgb(234, 237, 242),
    line: Color32::from_rgb(218, 222, 230),
    text: Color32::from_rgb(22, 25, 32),
    muted: Color32::from_rgb(88, 95, 108),
    faint: Color32::from_rgb(132, 139, 152),
    brand: Color32::from_rgb(6, 80, 236),
    brand_navy: Color32::from_rgb(17, 40, 76),
    ok: Color32::from_rgb(22, 138, 78),
    warn: Color32::from_rgb(178, 104, 0),
    info: Color32::from_rgb(28, 98, 222),
    error: Color32::from_rgb(200, 48, 48),
    banner: Color32::from_rgb(232, 239, 253),
    field: Color32::from_rgb(255, 255, 255),
    hover: Color32::from_rgb(226, 230, 237),
    hover_line: Color32::from_rgb(170, 177, 190),
    active: Color32::from_rgb(214, 219, 228),
};

static DARK_MODE: AtomicBool = AtomicBool::new(true);

/// Paleta del tema en uso.
pub fn pal() -> &'static Palette {
    if is_dark() {
        &DARK
    } else {
        &LIGHT
    }
}

pub fn is_dark() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

/// Cambia al tema claro u oscuro (paleta y visuales de egui).
pub fn apply(ctx: &egui::Context, dark: bool) {
    DARK_MODE.store(dark, Ordering::Relaxed);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

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

    ctx.set_visuals_of(egui::Theme::Dark, visuals(&DARK, true));
    ctx.set_visuals_of(egui::Theme::Light, visuals(&LIGHT, false));
    apply(ctx, true);
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

fn visuals(p: &Palette, dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.line);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.surface;
    v.hyperlink_color = p.text;
    v.selection.bg_fill = p.brand;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
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
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.muted);
    v.widgets.inactive.weak_bg_fill = p.surface_hi;
    v.widgets.inactive.bg_fill = p.surface_hi;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.hover_line);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.weak_bg_fill = p.active;
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.open.weak_bg_fill = p.active;
    v
}

/// Iconos de las apps que publican uno (el resto se dibuja con [`paint_drawn_icon`]) y la
/// marca de Iurefficient (icono y logotipo, rasterizados de `assets/brand` por `build.rs`).
pub struct Icons {
    textures: HashMap<&'static str, TextureHandle>,
    /// Logotipo con letras claras (tema oscuro) y con letras oscuras (tema claro).
    logo: Option<TextureHandle>,
    logo_light: Option<TextureHandle>,
}

/// Con mipmaps: la marca se dibuja bastante más pequeña que su PNG y así no se ve dentada.
const SHARP_DOWNSCALE: TextureOptions = TextureOptions {
    mipmap_mode: Some(egui::TextureFilter::Linear),
    ..TextureOptions::LINEAR
};

impl Icons {
    pub fn load(ctx: &egui::Context) -> Self {
        let files: [(&'static str, &'static [u8]); 7] = [
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
            (
                "iuredesk",
                include_bytes!("../../assets/icons/iuredesk.png"),
            ),
            ("iureapps", include_bytes!("../../assets/iureapps.png")),
            (
                "iurefficient",
                include_bytes!(concat!(env!("OUT_DIR"), "/icono.png")),
            ),
        ];
        let textures = files
            .into_iter()
            .filter_map(|(id, bytes)| {
                let img = decode_png(bytes)?;
                Some((id, ctx.load_texture(id, img, SHARP_DOWNSCALE)))
            })
            .collect();
        let logo = decode_png(include_bytes!(concat!(env!("OUT_DIR"), "/logotipo.png")))
            .map(|img| ctx.load_texture("logotipo", img, SHARP_DOWNSCALE));
        let logo_light = decode_png(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/logotipo-claro.png"
        )))
        .map(|img| ctx.load_texture("logotipo-claro", img, SHARP_DOWNSCALE));
        Icons {
            textures,
            logo,
            logo_light,
        }
    }

    pub fn paint(&self, painter: &Painter, rect: Rect, app_id: &str, accent: Color32) {
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        match self.textures.get(app_id) {
            // El icono de Iurefficient es sólo el trazo: va sobre el fondo de la suite.
            Some(t) if app_id == "iurefficient" => {
                let s = rect.width();
                painter.rect_filled(rect, CornerRadius::same((s * 0.22) as u8), pal().brand_navy);
                painter.image(t.id(), rect.shrink(s * 0.2), uv, Color32::WHITE);
            }
            Some(t) => {
                painter.image(t.id(), rect, uv, Color32::WHITE);
            }
            None => paint_drawn_icon(painter, rect, app_id, accent),
        }
    }

    /// Textura del icono de `app_id`, si la app publica uno.
    pub fn texture(&self, app_id: &str) -> Option<&TextureHandle> {
        self.textures.get(app_id)
    }

    /// Logotipo de Iurefficient de `height` puntos de alto (el texto «Iurefficient» si faltara).
    pub fn logo(&self, ui: &mut egui::Ui, height: f32) -> egui::Response {
        let logo = if is_dark() {
            &self.logo
        } else {
            &self.logo_light
        };
        match logo {
            Some(t) => {
                let [w, h] = t.size();
                let size = vec2(height * w as f32 / h as f32, height);
                ui.add(egui::Image::new((t.id(), size)))
            }
            None => ui.label(egui::RichText::new("Iurefficient").font(bold_font(height))),
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

/// Decodifica una captura de pantalla (PNG); `None` si no lo es o es desmesurada.
pub fn decode_shot(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = decode_png(bytes)?;
    (img.size[0] <= 4096 && img.size[1] <= 4096 && img.size[0] * img.size[1] > 0).then_some(img)
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
    painter.rect_filled(rect, CornerRadius::same((s * 0.22) as u8), pal().brand_navy);
    // Semicírculo superior (la «cúpula» del logotipo).
    let c = pos2(rect.center().x, rect.top() + s * 0.12);
    let r = s * 0.26;
    let points: Vec<_> = (0..=24)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / 24.0;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    painter.add(Shape::convex_polygon(points, pal().brand, Stroke::NONE));

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
        "iureti" => {
            // Red: un nodo central (la sonda) unido a cuatro equipos.
            let hub = pos2(rect.center().x, base + s * 0.17);
            let r_node = s * 0.055;
            let nodes = [(-0.24, -0.10), (0.24, -0.10), (-0.24, 0.16), (0.24, 0.16)];
            for (dx, dy) in nodes {
                let p = pos2(hub.x + s * dx, hub.y + s * dy);
                painter.line_segment([hub, p], Stroke::new(s * 0.025, white));
                painter.circle_filled(p, r_node, white);
            }
            painter.circle_filled(hub, r_node * 1.5, accent);
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
