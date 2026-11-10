//! A small call that shows what a quoted Typst function draws.
//!
//! A library compiles to an empty page, so the quote's function is called
//! against the file it lives in and that call is what the reviewer sees.

use std::path::Path;

use super::signature::{Function, functions_in, skip_string};

/// True when the file only defines bindings. Compiling it draws an empty page.
pub(super) fn is_library(text: &str) -> bool {
    let mut braces = 0i32;
    let mut parens = 0i32;
    for line in text.lines() {
        if braces == 0 && parens == 0 && !definition(line.trim()) {
            return false;
        }
        let (brace_delta, paren_delta) = depth_delta(line);
        braces += brace_delta;
        parens += paren_delta;
    }
    braces == 0 && parens == 0
}

fn definition(line: &str) -> bool {
    line.is_empty()
        || line.starts_with("//")
        || line.starts_with("#let ")
        || line.starts_with("#import ")
        || line.starts_with("#include ")
        || line.starts_with("#set ")
        || line.starts_with("#show ")
}

/// `(label, svg)` for a call of the functions in `source`, or nothing when
/// that call cannot be drawn.
pub(super) fn draw(project: &Path, path: &str, source: &str) -> Option<(String, String)> {
    if !super::render::typst_available() {
        return None;
    }
    let import = import_path(path)?;
    let functions = functions_in(source);
    if functions.is_empty() {
        return None;
    }
    drawn(project, &import, &functions).or_else(|| {
        functions
            .iter()
            .find_map(|function| drawn(project, &import, std::slice::from_ref(function)))
    })
}

fn drawn(project: &Path, import: &str, functions: &[Function]) -> Option<(String, String)> {
    let svg = super::render::compile_project(project, &document(import, functions))?;
    super::render::draws(&svg).then(|| (label(functions), svg))
}

fn document(import: &str, functions: &[Function]) -> String {
    let names = functions
        .iter()
        .map(|function| function.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut source =
        format!("#set page(width: auto, height: auto, margin: 12pt)\n#import {import}: {names}\n");
    for function in functions {
        let args = function
            .required
            .iter()
            .map(|name| sample(name))
            .collect::<Vec<_>>()
            .join(", ");
        source.push_str(&format!("#{}({args})\n#v(8pt)\n", function.name));
    }
    source
}

fn label(functions: &[Function]) -> String {
    let names = functions
        .iter()
        .map(|function| function.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!("Example of {names}")
}

fn import_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.split('/').any(|part| part == "..") {
        return None;
    }
    let path = path.trim_start_matches('/');
    Some(format!("\"/{}\"", path.replace('"', "\\\"")))
}

fn sample(name: &str) -> String {
    let name = name.to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "points" | "pts" | "vertices" | "coords" | "path"
    ) || name.ends_with("points")
    {
        "((0pt, 16pt), (48pt, 0pt), (96pt, 20pt))".to_owned()
    } else if name.contains("point")
        || matches!(name.as_str(), "at" | "origin" | "pos" | "from" | "to")
    {
        "(24pt, 12pt)".to_owned()
    } else if name.contains("paint")
        || name.contains("color")
        || name.contains("fill")
        || name == "stroke"
    {
        "rgb(\"#0072B2\")".to_owned()
    } else if name.contains("content") || matches!(name.as_str(), "body" | "child" | "it") {
        "[Example]".to_owned()
    } else if name.contains("label")
        || name.contains("title")
        || name.contains("text")
        || matches!(name.as_str(), "name" | "caption")
    {
        "\"Example\"".to_owned()
    } else if name.contains("width")
        || name.contains("height")
        || name.contains("length")
        || name.contains("size")
        || name.contains("radius")
        || name.contains("thick")
    {
        "12pt".to_owned()
    } else {
        "1".to_owned()
    }
}

fn depth_delta(line: &str) -> (i32, i32) {
    let bytes = line.as_bytes();
    let mut braces = 0i32;
    let mut parens = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            break;
        }
        if bytes[index] == b'"' {
            index = skip_string(bytes, index);
            continue;
        }
        match bytes[index] {
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b'(' => parens += 1,
            b')' => parens -= 1,
            _ => {}
        }
        index += 1;
    }
    (braces, parens)
}

#[cfg(test)]
mod tests {
    use super::{functions_in, is_library};

    #[test]
    fn a_route_signature_needs_only_its_points() {
        let text = "#let route(points, paint: ink, thickness: sw-base, dash: \"solid\", head: true) = {\n  let t = thickness / 1pt\n}\n";
        let functions = functions_in(text);
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].name, "route");
        assert_eq!(functions[0].required, ["points"]);
        assert!(is_library(text));
    }

    #[test]
    fn a_top_level_call_is_a_document() {
        let text = "#let ink = rgb(\"243746\")\n#route(((0pt, 0pt), (10pt, 10pt)))\n";
        assert!(!is_library(text));
    }
}
