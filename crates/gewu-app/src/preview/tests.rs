use std::path::Path;

use super::picture;
use crate::snapshot::{QuoteLine, QuoteLineKind, QuoteView};

fn quote(text: &str) -> QuoteView {
    QuoteView {
        change_id: "c".into(),
        path: "diagrams/lib/illustration.typ".into(),
        start: 1,
        end: 1,
        language: Some("typst".into()),
        more_above: false,
        more_below: false,
        lines: text
            .lines()
            .map(|line| QuoteLine {
                kind: QuoteLineKind::Added,
                old_line: None,
                new_line: Some(1),
                text: line.to_owned(),
            })
            .collect(),
        preview: None,
    }
}

#[test]
fn a_color_library_quote_shows_a_labeled_sheet() {
    let quote = quote("#let ink = rgb(\"243746\")\n#let quiet = rgb(\"D2DCE3\")\n");
    let preview = picture(Path::new("/no/such/root"), &quote).expect("preview");
    assert_eq!(preview.media, "image/svg+xml");
    assert!(
        preview.data.contains("243746") || preview.data.contains("ink"),
        "{preview:?}"
    );
    assert!(
        preview.label.contains("color") || preview.label.contains("Rendered"),
        "{preview:?}"
    );
}

#[test]
fn a_sibling_svg_is_the_rendered_output() {
    let dir = super::render::scratch_dir().expect("temp");
    std::fs::write(dir.join("card.typ"), "= Hello\n").unwrap();
    std::fs::write(
        dir.join("card.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"4\" height=\"4\"/></svg>",
    )
    .unwrap();
    let mut quote = quote("= Hello\n");
    quote.path = "card.typ".into();
    let preview = picture(&dir, &quote).expect("preview");
    assert_eq!(preview.label, "Rendered output");
    assert!(preview.data.contains("<rect"), "{preview:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_uncommitted_render_in_the_repository_is_shown_from_a_worktree() {
    let tmp = super::render::scratch_dir().expect("temp");
    let main = tmp.join("repo");
    let gitdir = main.join(".git/worktrees/task");
    std::fs::create_dir_all(&gitdir).unwrap();
    std::fs::write(gitdir.join("commondir"), "../..\n").unwrap();
    let task = tmp.join("task");
    std::fs::create_dir_all(task.join("diagrams")).unwrap();
    std::fs::write(task.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
    std::fs::write(task.join("diagrams/card.typ"), "= Hello\n").unwrap();
    std::fs::create_dir_all(main.join("diagrams")).unwrap();
    std::fs::write(
        main.join("diagrams/card.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"4\" height=\"4\"/></svg>",
    )
    .unwrap();
    let mut shown = quote("= Hello\n");
    shown.path = "diagrams/card.typ".into();
    let preview = picture(&task, &shown).expect("preview");
    assert_eq!(preview.label, "Rendered output");
    assert!(preview.data.contains("<rect"), "{preview:?}");
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn a_blank_page_is_not_a_drawing() {
    let blank =
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><path fill=\"#ffffff\" d=\"M 0 0\"/></svg>";
    assert!(!super::render::draws(blank));
    assert!(super::render::draws(
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><path fill=\"#243746\" d=\"M 0 0\"/></svg>"
    ));
}

#[test]
fn a_placed_grid_is_shown_in_full() {
    let tmp = super::render::scratch_dir().expect("temp");
    let lib = tmp.join("diagrams/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("illustration.typ"),
        "\
#let accent = rgb(\"0072B2\")
#let dot(x, y, r: 3.5, fill: accent, paint: accent) = {
  place(dx: x * 1pt, dy: y * 1pt, circle(radius: r * 1pt, fill: paint))
}
#let atoms(x, y, nx: 4, ny: 3, step: 12, r: 3.5, fill: accent) = {
  for i in range(nx) {
    for j in range(ny) {
      dot(x + i * step, y + j * step, r: r, fill: fill, paint: fill)
    }
  }
}
",
    )
    .unwrap();
    let mut shown = quote(
        "#let atoms(x, y, nx: 4, ny: 3, step: 12, r: 3.5, fill: accent) = {\n  dot(x, y)\n}\n",
    );
    shown.path = "diagrams/lib/illustration.typ".into();
    let preview = picture(&tmp, &shown);
    if super::render::typst_available() {
        let preview = preview.expect("example");
        assert_eq!(preview.label, "Example of atoms");
        let view = preview
            .data
            .split("viewBox=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or("");
        let nums: Vec<f64> = view
            .split_whitespace()
            .filter_map(|part| part.parse().ok())
            .collect();
        assert_eq!(nums.len(), 4, "{view}");
        let (x, y, width, height) = (nums[0], nums[1], nums[2], nums[3]);
        assert!(
            x + width > 49.0 && y + height > 37.0,
            "viewport {view} cuts off the grid"
        );
        assert!(
            !preview.data.to_ascii_lowercase().contains("ffffff"),
            "the page fill is smaller than the grid"
        );
    } else {
        assert!(preview.is_none());
    }
    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn a_route_function_is_drawn_instead_of_a_blank_page() {
    let tmp = super::render::scratch_dir().expect("temp");
    let lib = tmp.join("diagrams/lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("illustration.typ"),
        "\
#let ink = rgb(\"243746\")
#let sw-base = 1.5pt
#let route(points, paint: ink, thickness: sw-base, dash: \"solid\", head: true) = {
  if points.len() < 2 { return [] }
  curve(
    stroke: paint + thickness,
    curve.move(points.first()),
    ..points.slice(1).map(p => curve.line(p)),
  )
}
",
    )
    .unwrap();
    std::fs::write(
        lib.join("illustration.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><path fill=\"#ffffff\" d=\"M 0 0\"/></svg>",
    )
    .unwrap();
    let mut shown = quote(
        "#let route(points, paint: ink, thickness: sw-base, dash: \"solid\", head: true) = {\n",
    );
    shown.path = "diagrams/lib/illustration.typ".into();
    let preview = picture(&tmp, &shown);
    if super::render::typst_available() {
        let preview = preview.expect("example");
        assert_eq!(preview.label, "Example of route");
        assert!(super::render::draws(&preview.data), "{preview:?}");
    } else {
        assert!(preview.is_none());
    }
    let _ = std::fs::remove_dir_all(tmp);
}
