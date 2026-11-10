//! A picture of what a Typst quote produces, beside the source.
//!
//! A color library draws nothing by itself, so the quote's bindings become a
//! labeled sheet. A document, or a PNG/SVG/PDF sitting next to the file
//! (committed or not), is shown instead.

mod colors;
mod example;
mod frame;
mod paint;
mod render;
mod sheet;
mod signature;

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::snapshot::{BlockView, QuoteLineKind, QuotePreview, QuoteView, Snapshot};

use colors::Swatch;

const LABEL_RENDERED: &str = "Rendered output";
const LABEL_COLORS: &str = "What these colors look like";

/// Fill `preview` on Typst quotes. Never fails the snapshot: a quote with no
/// picture simply has none.
pub(crate) fn attach(root: &Path, snapshot: &mut Snapshot) {
    for chunk in &mut snapshot.chunks {
        for block in &mut chunk.blocks {
            let BlockView::Changes { quotes, .. } = block else {
                continue;
            };
            for quote in quotes.iter_mut() {
                if is_typst(quote) {
                    quote.preview = picture(root, quote);
                }
            }
        }
    }
}

fn is_typst(quote: &QuoteView) -> bool {
    quote.language.as_deref() == Some("typst") || quote.path.ends_with(".typ")
}

fn picture(root: &Path, quote: &QuoteView) -> Option<QuotePreview> {
    if let Some(rendered) = sibling(root, &quote.path)
        && shows_a_drawing(&rendered)
    {
        return Some(rendered);
    }
    let file = root.join(&quote.path);
    let on_disk = fs::read_to_string(&file).ok();
    if on_disk
        .as_ref()
        .is_some_and(|text| !example::is_library(text))
        && let Some(svg) = render::compile_file(&file).filter(|svg| render::draws(svg))
    {
        return Some(preview(LABEL_RENDERED, "image/svg+xml", &svg));
    }
    let source = quote
        .lines
        .iter()
        .filter(|line| line.kind != QuoteLineKind::Removed)
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let project = library_root(root, &quote.path);
    if let Some(project) = project
        && let Some((label, svg)) = example::draw(&project, &quote.path, &source)
    {
        return Some(preview(&label, "image/svg+xml", &svg));
    }
    let swatches = colors::swatches_in(&source);
    if swatches.is_empty() {
        return None;
    }
    let svg = example_svg(&source, &swatches).unwrap_or_else(|| sheet::sheet(&swatches));
    Some(preview(LABEL_COLORS, "image/svg+xml", &svg))
}

/// An SVG that is only a blank page is not a result. A PNG or PDF the author
/// rendered is shown as they left it.
fn shows_a_drawing(preview: &QuotePreview) -> bool {
    preview.media != "image/svg+xml" || render::draws(&preview.data)
}

fn library_root(root: &Path, path: &str) -> Option<PathBuf> {
    if root.join(path).is_file() {
        return Some(root.to_path_buf());
    }
    let origin = origin_checkout(root)?;
    origin.join(path).is_file().then_some(origin)
}

/// `{stem}.svg`, `{stem}.png`, or `{stem}.pdf` beside the source. A render the
/// author never committed still sits in the repository, while a commit review
/// is checked out in its own worktree, so look there too.
fn sibling(root: &Path, path: &str) -> Option<QuotePreview> {
    beside(root, path).or_else(|| origin_checkout(root).and_then(|origin| beside(&origin, path)))
}

fn beside(root: &Path, path: &str) -> Option<QuotePreview> {
    let file = root.join(path);
    let stem = file.file_stem()?.to_str()?;
    let dir = file.parent()?;
    for extension in ["svg", "png", "pdf"] {
        let candidate = dir.join(format!("{stem}.{extension}"));
        if candidate.is_file()
            && let Some(preview) = read_render(&candidate)
        {
            return Some(preview);
        }
    }
    None
}

/// The repository a linked worktree was added from.
fn origin_checkout(root: &Path) -> Option<PathBuf> {
    let link = fs::read_to_string(root.join(".git")).ok()?;
    let gitdir = PathBuf::from(link.trim().strip_prefix("gitdir:")?.trim());
    let relative = fs::read_to_string(gitdir.join("commondir")).ok()?;
    let common = gitdir.join(relative.trim()).canonicalize().ok()?;
    let origin = common.parent()?.to_path_buf();
    (origin != root).then_some(origin)
}

fn read_render(path: &Path) -> Option<QuotePreview> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("svg") => {
            let svg = render::sanitize_svg(&fs::read_to_string(path).ok()?)?;
            Some(preview(LABEL_RENDERED, "image/svg+xml", &svg))
        }
        Some("png") => Some(preview(
            LABEL_RENDERED,
            "image/png",
            &render::base64(&fs::read(path).ok()?),
        )),
        Some("pdf") => render::rasterize_pdf(path),
        _ => None,
    }
}

fn example_svg(source: &str, swatches: &[Swatch]) -> Option<String> {
    let named: Vec<_> = swatches
        .iter()
        .filter(|swatch| swatch.name.is_some())
        .collect();
    if named.is_empty() || !render::typst_available() {
        return None;
    }
    let mut document = String::from("#set page(width: auto, height: auto, margin: 12pt)\n");
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#let ") {
            document.push_str(line);
            document.push('\n');
        }
    }
    document.push_str(
        "#let sample(label, paint) = grid(columns: (16pt, auto), column-gutter: 8pt, \
         box(width: 16pt, height: 16pt, fill: paint, stroke: 0.5pt + rgb(\"888888\"), radius: 2pt), label)\n",
    );
    for swatch in named {
        let Some(name) = swatch.name.as_deref() else {
            continue;
        };
        document.push_str(&format!("#sample(\"{name}\", {name})\n"));
    }
    render::compile_source(&document)
}

fn preview(label: &str, media: &str, data: &str) -> QuotePreview {
    QuotePreview {
        label: label.to_owned(),
        media: media.to_owned(),
        data: data.to_owned(),
    }
}

#[cfg(test)]
mod tests;
