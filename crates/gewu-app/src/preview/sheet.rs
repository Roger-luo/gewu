use super::colors::Swatch;

/// A labeled sheet of `swatches`, the example a palette library does not draw itself.
pub(crate) fn sheet(swatches: &[Swatch]) -> String {
    let row = 28;
    let height = 16 + row * swatches.len();
    let mut body = String::new();
    for (index, swatch) in swatches.iter().enumerate() {
        let y = 12 + index * row;
        let label = swatch.name.as_deref().unwrap_or(swatch.hex.as_str());
        body.push_str(&format!(
            r##"<rect x="12" y="{y}" width="16" height="16" rx="2" fill="{hex}" stroke="#888888" stroke-width="1"/>"##,
            hex = swatch.hex,
        ));
        body.push_str(&format!(
            r##"<text x="38" y="{text_y}" font-family="ui-sans-serif, sans-serif" font-size="13" fill="#243746">{label}</text>"##,
            text_y = y + 13,
            label = xml_text(label),
        ));
        body.push_str(&format!(
            r##"<text x="150" y="{text_y}" font-family="ui-monospace, monospace" font-size="12" fill="#526575">{hex}</text>"##,
            text_y = y + 13,
            hex = swatch.hex,
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="{height}" viewBox="0 0 240 {height}" role="img">{body}</svg>"##
    )
}

fn xml_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::super::colors::Swatch;
    use super::sheet;

    #[test]
    fn the_sheet_names_the_color_and_paints_it() {
        let svg = sheet(&[Swatch {
            name: Some("ink".into()),
            hex: "#243746".into(),
        }]);
        assert!(svg.contains("ink"), "{svg}");
        assert!(svg.contains("#243746"), "{svg}");
        assert!(svg.starts_with("<svg"), "{svg}");
    }
}
