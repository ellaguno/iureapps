//! Markdown sencillo para las novedades de cada versión (el cuerpo de la release de GitHub):
//! títulos, párrafos, listas (con anidación), citas, bloques de código, separadores, negrita,
//! cursiva, código, enlaces y URL sueltas. Las URL de GitHub se acortan como lo hace GitHub
//! (`…/pull/7` → `#7`). Lo que no reconoce se muestra como texto.

use super::theme::{self, pal};
use eframe::egui::{self, RichText, Sense, Stroke, Ui};

const BODY: f32 = 13.5;

/// Trozo de texto con su estilo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Span>),
    Para(Vec<Span>),
    /// Elemento de lista: nivel de anidación, viñeta («•» o «3.») y texto.
    Item(usize, String, Vec<Span>),
    Quote(Vec<Span>),
    Code(String),
    Rule,
    /// Tabla al estilo de GitHub: encabezado y filas (cada celda con su formato).
    Table(Vec<Vec<Span>>, Vec<Vec<Vec<Span>>>),
}

/// Celdas de una fila de tabla (`| a | b |`), o `None` si la línea no tiene `|`. Respeta
/// `\|` y las barras dentro de `código`.
fn table_cells(line: &str) -> Option<Vec<String>> {
    let line = line.trim();
    if !line.contains('|') {
        return None;
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut code = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '`' => {
                code = !code;
                cur.push(c);
            }
            '|' if !code => cells.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    cells.push(cur);
    // Las barras de los extremos dejan celdas vacías que no cuentan.
    if line.starts_with('|') {
        cells.remove(0);
    }
    if line.ends_with('|') && !line.ends_with("\\|") {
        cells.pop();
    }
    Some(cells.into_iter().map(|c| c.trim().to_string()).collect())
}

/// ¿Es la fila separadora (`| --- | :---: |`) de una tabla de `columns` columnas?
fn is_table_separator(line: &str, columns: usize) -> bool {
    table_cells(line).is_some_and(|cells| {
        cells.len() == columns
            && cells.iter().all(|c| {
                let d = c.trim_start_matches(':').trim_end_matches(':');
                !d.is_empty() && d.chars().all(|ch| ch == '-')
            })
    })
}

/// Quita los comentarios HTML (`<!-- … -->`), que GitHub mete en las notas generadas.
fn strip_comments(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut rest = md;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Viñeta de lista al principio de `line` (ya sin sangría): («•» o «3.», resto).
fn list_marker(line: &str) -> Option<(String, &str)> {
    for b in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(b) {
            return Some(("•".into(), rest));
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if (1..=3).contains(&digits) {
        let after = &line[digits..];
        if let Some(rest) = after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))
        {
            return Some((format!("{}.", &line[..digits]), rest));
        }
    }
    None
}

fn is_rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3
        && (t.chars().all(|c| c == '-')
            || t.chars().all(|c| c == '*')
            || t.chars().all(|c| c == '_'))
}

pub fn parse(md: &str) -> Vec<Block> {
    let md = strip_comments(md);
    let mut blocks = Vec::new();
    // Texto que se va juntando: párrafo, cita o el último elemento de lista.
    let mut para: Vec<String> = Vec::new();
    let mut quote: Vec<String> = Vec::new();
    let mut item: Option<(usize, String, Vec<String>)> = None;
    let mut code: Option<Vec<String>> = None;

    fn flush(
        blocks: &mut Vec<Block>,
        para: &mut Vec<String>,
        quote: &mut Vec<String>,
        item: &mut Option<(usize, String, Vec<String>)>,
    ) {
        if !para.is_empty() {
            blocks.push(Block::Para(inline(&para.join(" "))));
            para.clear();
        }
        if !quote.is_empty() {
            blocks.push(Block::Quote(inline(&quote.join(" "))));
            quote.clear();
        }
        if let Some((level, marker, lines)) = item.take() {
            blocks.push(Block::Item(level, marker, inline(&lines.join(" "))));
        }
    }

    let lines: Vec<&str> = md.lines().collect();
    let mut k = 0;
    while k < lines.len() {
        let line = lines[k].trim_end();
        k += 1;
        if let Some(lines) = code.as_mut() {
            if line.trim_start().starts_with("```") {
                blocks.push(Block::Code(lines.join("\n")));
                code = None;
            } else {
                lines.push(line.to_string());
            }
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        let text = line.trim_start();
        if text.starts_with("```") {
            flush(&mut blocks, &mut para, &mut quote, &mut item);
            code = Some(Vec::new());
            continue;
        }
        if text.is_empty() {
            flush(&mut blocks, &mut para, &mut quote, &mut item);
            continue;
        }
        // Tabla: fila de encabezado seguida de la separadora.
        if let Some(header) = table_cells(text) {
            let columns = header.len();
            if columns > 0 && lines.get(k).is_some_and(|l| is_table_separator(l, columns)) {
                flush(&mut blocks, &mut para, &mut quote, &mut item);
                k += 1;
                let mut rows = Vec::new();
                while let Some(mut row) = lines
                    .get(k)
                    .filter(|l| !l.trim().is_empty())
                    .and_then(|l| table_cells(l))
                {
                    row.resize(columns, String::new());
                    rows.push(row.iter().map(|c| inline(c)).collect());
                    k += 1;
                }
                let header = header.iter().map(|c| inline(c)).collect();
                blocks.push(Block::Table(header, rows));
                continue;
            }
        }
        let hashes = text.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && text[hashes..].starts_with(' ') {
            flush(&mut blocks, &mut para, &mut quote, &mut item);
            let title = text[hashes..].trim().trim_end_matches('#').trim();
            blocks.push(Block::Heading(hashes as u8, inline(title)));
            continue;
        }
        if is_rule(text) {
            flush(&mut blocks, &mut para, &mut quote, &mut item);
            blocks.push(Block::Rule);
            continue;
        }
        if let Some((marker, rest)) = list_marker(text) {
            flush(&mut blocks, &mut para, &mut quote, &mut item);
            item = Some((indent / 2, marker, vec![rest.to_string()]));
            continue;
        }
        if let Some(rest) = text.strip_prefix('>') {
            if !para.is_empty() || item.is_some() {
                flush(&mut blocks, &mut para, &mut quote, &mut item);
            }
            quote.push(rest.trim().to_string());
            continue;
        }
        // Continuación del elemento de lista anterior, o texto normal.
        match item.as_mut() {
            Some((_, _, lines)) if indent > 0 => lines.push(text.to_string()),
            _ => {
                if item.is_some() || !quote.is_empty() {
                    flush(&mut blocks, &mut para, &mut quote, &mut item);
                }
                para.push(text.to_string());
            }
        }
    }
    if let Some(lines) = code {
        blocks.push(Block::Code(lines.join("\n")));
    }
    flush(&mut blocks, &mut para, &mut quote, &mut item);
    blocks
}

/// Texto visible de una URL: como GitHub, `#7` para pull requests e issues, `a...b` para
/// comparaciones y el SHA corto para commits; si no, la URL sin `https://`.
pub fn short_url(url: &str) -> String {
    if let Some(path) = url.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = path.split('/').collect();
        match parts.as_slice() {
            [_, _, "pull" | "issues", n, ..] if n.chars().all(|c| c.is_ascii_digit()) => {
                return format!("#{n}")
            }
            [_, _, "compare", range] => return (*range).to_string(),
            [_, _, "commit", sha] if sha.len() >= 7 => return sha[..7].to_string(),
            _ => {}
        }
    }
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

/// Formato en línea: `**negrita**`, `*cursiva*`, `` `código` ``, `[texto](url)`,
/// `<https://…>` y URL sueltas.
pub fn inline(text: &str) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut spans: Vec<Span> = Vec::new();
    let mut cur = String::new();
    let (mut bold, mut italic) = (false, false);
    let mut i = 0;

    let push = |spans: &mut Vec<Span>, cur: &mut String, bold: bool, italic: bool| {
        if !cur.is_empty() {
            spans.push(Span {
                text: std::mem::take(cur),
                bold,
                italic,
                ..Default::default()
            });
        }
    };
    let find = |from: usize, c: char| chars[from..].iter().position(|x| *x == c).map(|p| p + from);

    while i < chars.len() {
        let c = chars[i];
        let rest: String = chars[i..].iter().take(8).collect();
        if c == '\\' && i + 1 < chars.len() && chars[i + 1].is_ascii_punctuation() {
            cur.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '`' {
            if let Some(end) = find(i + 1, '`') {
                push(&mut spans, &mut cur, bold, italic);
                spans.push(Span {
                    text: chars[i + 1..end].iter().collect(),
                    code: true,
                    bold,
                    italic,
                    url: None,
                });
                i = end + 1;
                continue;
            }
        }
        if rest.starts_with("**") || rest.starts_with("__") {
            push(&mut spans, &mut cur, bold, italic);
            bold = !bold;
            i += 2;
            continue;
        }
        // `*` de cursiva sólo pegado a una palabra (no «2 * 3»).
        if c == '*' {
            let next = chars.get(i + 1).is_some_and(|n| !n.is_whitespace());
            let prev = i > 0 && !chars[i - 1].is_whitespace();
            if (italic && prev) || (!italic && next) {
                push(&mut spans, &mut cur, bold, italic);
                italic = !italic;
                i += 1;
                continue;
            }
        }
        // [texto](url) e ![imagen](url)
        let image = c == '!' && chars.get(i + 1) == Some(&'[');
        if c == '[' || image {
            let open = if image { i + 1 } else { i };
            if let Some(close) = find(open + 1, ']') {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = find(close + 2, ')') {
                        let label: String = chars[open + 1..close].iter().collect();
                        let url: String = chars[close + 2..end].iter().collect();
                        let label = inline(&label)
                            .into_iter()
                            .map(|s| s.text)
                            .collect::<String>();
                        let label = match (image, label.trim().is_empty()) {
                            (true, true) => "imagen".to_string(),
                            (true, false) => format!("imagen: {label}"),
                            (false, true) => short_url(&url),
                            (false, false) => label,
                        };
                        push(&mut spans, &mut cur, bold, italic);
                        spans.push(Span {
                            text: label,
                            bold,
                            italic,
                            url: Some(url.trim().to_string()),
                            ..Default::default()
                        });
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        // <https://…>
        if c == '<' && (rest.starts_with("<https:") || rest.starts_with("<http:")) {
            if let Some(end) = find(i + 1, '>') {
                let url: String = chars[i + 1..end].iter().collect();
                push(&mut spans, &mut cur, bold, italic);
                spans.push(Span {
                    text: short_url(&url),
                    bold,
                    italic,
                    url: Some(url),
                    ..Default::default()
                });
                i = end + 1;
                continue;
            }
        }
        // URL suelta, sin la puntuación final.
        if rest.starts_with("https://") || rest.starts_with("http://") {
            let len = chars[i..]
                .iter()
                .take_while(|c| !c.is_whitespace() && **c != '<' && **c != '>')
                .count();
            let mut url: String = chars[i..i + len].iter().collect();
            while url.ends_with(['.', ',', ';', ':', ')', '!', '?']) {
                url.pop();
            }
            let used = url.chars().count();
            push(&mut spans, &mut cur, bold, italic);
            spans.push(Span {
                text: short_url(&url),
                bold,
                italic,
                url: Some(url),
                ..Default::default()
            });
            i += used;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    push(&mut spans, &mut cur, bold, italic);
    spans
}

fn rich(span: &Span, size: f32, color: egui::Color32) -> RichText {
    let mut t = RichText::new(&span.text).size(size).color(color);
    if span.code {
        t = t
            .font(theme::mono(size - 1.0))
            .background_color(pal().surface_hi)
            .color(pal().text);
    } else if span.bold {
        t = t.family(theme::bold());
    }
    if span.italic {
        t = t.italics();
    }
    t
}

/// Una línea de texto con formato, que se parte donde haga falta.
fn spans_ui(ui: &mut Ui, spans: &[Span], size: f32, color: egui::Color32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        for s in spans {
            match &s.url {
                Some(url) => {
                    let r = ui
                        .add(egui::Label::new(rich(s, size, pal().info)).sense(Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(url);
                    if r.hovered() {
                        let y = r.rect.bottom() - 1.0;
                        ui.painter()
                            .hline(r.rect.x_range(), y, Stroke::new(1.0, pal().info));
                    }
                    if r.clicked() {
                        super::open_link(url);
                    }
                }
                None => {
                    ui.label(rich(s, size, color));
                }
            }
        }
    });
}

/// Celda de tabla: un solo texto que se parte; si lleva un enlace, la celda lo abre.
fn table_cell(ui: &mut Ui, spans: &[Span], header: bool) {
    let mut job = egui::text::LayoutJob::default();
    for s in spans {
        let s = Span {
            bold: s.bold || header,
            ..s.clone()
        };
        let color = if s.url.is_some() {
            pal().info
        } else {
            pal().text
        };
        rich(&s, BODY - 0.5, color).append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Default,
            egui::Align::Center,
        );
    }
    let url = spans.iter().find_map(|s| s.url.clone());
    let label = egui::Label::new(job).wrap();
    match url {
        Some(url) => {
            if ui
                .add(label.sense(Sense::click()))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(&url)
                .clicked()
            {
                super::open_link(&url);
            }
        }
        None => {
            ui.add(label);
        }
    }
}

/// Dibuja `md` con el estilo del lanzador.
pub fn show(ui: &mut Ui, md: &str) {
    let blocks = parse(md);
    ui.spacing_mut().item_spacing.y = 6.0;
    for (k, block) in blocks.iter().enumerate() {
        match block {
            Block::Heading(level, spans) => {
                if k > 0 {
                    ui.add_space(8.0);
                }
                let size = match level {
                    1 => 18.0,
                    2 => 16.0,
                    _ => 14.5,
                };
                let spans: Vec<Span> = spans
                    .iter()
                    .map(|s| Span {
                        bold: true,
                        ..s.clone()
                    })
                    .collect();
                spans_ui(ui, &spans, size, pal().text);
            }
            Block::Para(spans) => spans_ui(ui, spans, BODY, pal().text),
            Block::Item(level, marker, spans) => {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(4.0 + 18.0 * *level as f32);
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(20.0, BODY + 4.0), Sense::hover());
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        marker,
                        theme::font(BODY),
                        pal().muted,
                    );
                    spans_ui(ui, spans, BODY, pal().text);
                });
            }
            Block::Quote(spans) => {
                let r = ui
                    .horizontal_top(|ui| {
                        ui.add_space(14.0);
                        spans_ui(ui, spans, BODY, pal().muted);
                    })
                    .response
                    .rect;
                ui.painter()
                    .vline(r.left() + 4.0, r.y_range(), Stroke::new(2.0, pal().line));
            }
            Block::Code(code) => {
                egui::Frame::new()
                    .fill(pal().surface_hi)
                    .inner_margin(10)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(code)
                                .font(theme::mono(12.0))
                                .color(pal().text),
                        );
                    });
            }
            Block::Table(header, rows) => {
                let columns = header.len().max(1) as f32;
                let max_col = ((ui.available_width() - 20.0) / columns * 1.8).max(80.0);
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, pal().line))
                    .inner_margin(8)
                    .show(ui, |ui| {
                        egui::Grid::new(("md-table", k))
                            .num_columns(header.len())
                            .striped(true)
                            .spacing([18.0, 6.0])
                            .max_col_width(max_col)
                            .show(ui, |ui| {
                                for c in header {
                                    table_cell(ui, c, true);
                                }
                                ui.end_row();
                                for row in rows {
                                    for c in row {
                                        table_cell(ui, c, false);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            }
            Block::Rule => {
                ui.add_space(2.0);
                let w = ui.available_width();
                let (r, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), Sense::hover());
                ui.painter()
                    .hline(r.x_range(), r.center().y, Stroke::new(1.0, pal().line));
                ui.add_space(2.0);
            }
        }
    }
    if blocks.is_empty() {
        ui.label(RichText::new("Sin notas.").size(BODY).color(pal().faint));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Notas como las genera GitHub con `--generate-notes`.
    const GITHUB: &str =
        "<!-- Release notes generated using configuration in .github/release.yml -->

## What's Changed
* Mosaico estilo GNOME by @ellaguno in https://github.com/ellaguno/iureapps/pull/7
* El lanzador se actualiza solo by @ellaguno in https://github.com/ellaguno/iureapps/pull/8


**Full Changelog**: https://github.com/ellaguno/iureapps/compare/v0.1.12...v0.1.13";

    #[test]
    fn notas_de_github() {
        let b = parse(GITHUB);
        assert_eq!(b.len(), 4, "{b:#?}");
        assert!(matches!(&b[0], Block::Heading(2, s) if text(s) == "What's Changed"));
        match &b[1] {
            Block::Item(0, m, s) => {
                assert_eq!(m, "•");
                assert_eq!(text(s), "Mosaico estilo GNOME by @ellaguno in #7");
                let link = s.iter().find(|s| s.url.is_some()).unwrap();
                assert_eq!(
                    link.url.as_deref(),
                    Some("https://github.com/ellaguno/iureapps/pull/7")
                );
            }
            other => panic!("{other:?}"),
        }
        match &b[3] {
            Block::Para(s) => {
                assert!(s[0].bold && s[0].text == "Full Changelog");
                assert_eq!(text(s), "Full Changelog: v0.1.12...v0.1.13");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn listas_anidadas_y_continuacion() {
        let b = parse("1. Uno\n   sigue\n2. Dos\n  - sub\n\nPárrafo\nen dos líneas");
        assert_eq!(
            b,
            vec![
                Block::Item(0, "1.".into(), inline("Uno sigue")),
                Block::Item(0, "2.".into(), inline("Dos")),
                Block::Item(1, "•".into(), inline("sub")),
                Block::Para(inline("Párrafo en dos líneas")),
            ]
        );
    }

    #[test]
    fn formato_en_linea() {
        let s = inline("Usa **`cargo`** y *esto*, ver [docs](https://x.com/a). 2 * 3");
        assert!(s.iter().any(|s| s.code && s.bold && s.text == "cargo"));
        assert!(s.iter().any(|s| s.italic && s.text == "esto"));
        assert!(s
            .iter()
            .any(|s| s.text == "docs" && s.url.as_deref() == Some("https://x.com/a")));
        assert!(text(&s).ends_with("2 * 3"));
        // URL suelta sin el punto final.
        let s = inline("Ver https://ejemplo.com/a.");
        assert_eq!(s[1].url.as_deref(), Some("https://ejemplo.com/a"));
        assert_eq!(text(&s), "Ver ejemplo.com/a.");
    }

    #[test]
    fn bloques_varios() {
        let b = parse("# Título\n> cita\n> sigue\n\n---\n```\nlet x = 1;\n```\n\\*literal\\*");
        assert_eq!(
            b,
            vec![
                Block::Heading(1, inline("Título")),
                Block::Quote(inline("cita sigue")),
                Block::Rule,
                Block::Code("let x = 1;".into()),
                Block::Para(vec![Span {
                    text: "*literal*".into(),
                    ..Default::default()
                }]),
            ]
        );
    }

    #[test]
    fn urls_cortas() {
        assert_eq!(short_url("https://github.com/o/r/issues/12"), "#12");
        assert_eq!(
            short_url("https://github.com/o/r/commit/0123456789abcdef"),
            "0123456"
        );
        assert_eq!(short_url("https://iurefficient.com/"), "iurefficient.com");
    }

    /// Notas como las de IureOCR: tabla con código en las celdas, luego un párrafo.
    #[test]
    fn tablas() {
        let md = "Elige el de tu sistema:\r\n\r\n| Archivo | Sistema |\r\n| --- | :---: |\r\n\
                  | `*_1-windows-x64.exe` / `.msi` | Windows 10/11 |\r\n\
                  | `a|b` | Linux \\| BSD | sobra |\r\n\r\nVer [CHANGELOG](https://x.com/c).";
        let b = parse(md);
        assert_eq!(b.len(), 3, "{b:#?}");
        match &b[1] {
            Block::Table(h, rows) => {
                assert_eq!(h.len(), 2);
                assert_eq!(text(&h[0]), "Archivo");
                assert_eq!(rows.len(), 2);
                assert!(rows[0][0][0].code);
                assert_eq!(text(&rows[0][0]), "*_1-windows-x64.exe / .msi");
                assert_eq!(text(&rows[1][0]), "a|b");
                assert_eq!(text(&rows[1][1]), "Linux | BSD");
                // Las celdas de más se descartan.
                assert_eq!(rows[1].len(), 2);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(&b[2], Block::Para(_)));
        // Sin fila separadora no es tabla.
        assert!(matches!(parse("a | b\nc | d").as_slice(), [Block::Para(_)]));
    }
}
