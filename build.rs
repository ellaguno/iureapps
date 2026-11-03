//! Rasteriza los SVG de marca (`assets/brand`) a PNG en `OUT_DIR` para incrustarlos: el
//! ejecutable no lleva un motor de SVG.

use resvg::{tiny_skia, usvg};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/brand");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out = Path::new(&out);
    // Logotipo para fondo oscuro: letras claras (en vez del azul marino) y la cúpula en azul.
    let logo = std::fs::read_to_string("assets/brand/logotipo.svg")
        .expect("assets/brand/logotipo.svg")
        .replace("#11295b", "#e8eaef");
    render(&logo, 192, true, &out.join("logotipo.png"));
    let icon = std::fs::read_to_string("assets/brand/icono-negativo.svg")
        .expect("assets/brand/icono-negativo.svg");
    render(&icon, 256, false, &out.join("icono.png"));
    windows_resources();
}

/// En Windows, el icono de la suite y los datos de la versión dentro del .exe (los usan el
/// Explorador, los accesos directos y el instalador).
fn windows_resources() {
    println!("cargo:rerun-if-changed=assets/iureapps.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/iureapps.ico")
            .set("ProductName", "Iurefficient Apps")
            .set("FileDescription", "Iurefficient Apps");
        res.compile()
            .expect("no se pudieron incrustar los recursos de Windows");
    }
}

/// `crop`: recorta el margen del SVG y deja sólo lo dibujado.
fn render(svg: &str, height: u32, crop: bool, path: &Path) {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("SVG inválido");
    let area = if crop {
        tree.root().abs_bounding_box()
    } else {
        let s = tree.size();
        usvg::Rect::from_xywh(0.0, 0.0, s.width(), s.height()).expect("tamaño del SVG")
    };
    let scale = height as f32 / area.height();
    let width = (area.width() * scale).ceil() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("tamaño del PNG");
    let transform =
        tiny_skia::Transform::from_translate(-area.x(), -area.y()).post_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.save_png(path).expect("no se pudo escribir el PNG");
}
