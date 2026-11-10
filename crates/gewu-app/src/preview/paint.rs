//! Whether an SVG is a picture or a blank page.

/// True when the SVG paints something other than a blank white page.
pub(super) fn draws(svg: &str) -> bool {
    if paint_attrs(svg, "fill=\"").any(paint_shows)
        || paint_attrs(svg, "stroke=\"").any(paint_shows)
    {
        return true;
    }
    let lower = svg.to_ascii_lowercase();
    [
        "<rect",
        "<line",
        "<polyline",
        "<polygon",
        "<circle",
        "<ellipse",
        "<image",
        "<text",
        "<use",
    ]
    .iter()
    .any(|tag| lower.contains(tag))
}

fn paint_attrs<'a>(svg: &'a str, attr: &'a str) -> impl Iterator<Item = &'a str> {
    svg.match_indices(attr).filter_map(move |(index, _)| {
        let rest = svg.get(index + attr.len()..)?;
        let end = rest.find('"')?;
        Some(&rest[..end])
    })
}

/// A page-colored fill. `none` is not paper; it is the absence of a fill.
pub(super) fn is_paper(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("transparent")
    {
        return false;
    }
    !paint_shows(value)
}

fn paint_shows(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.eq_ignore_ascii_case("none")
        || value.eq_ignore_ascii_case("transparent")
        || value.eq_ignore_ascii_case("white")
        || value.eq_ignore_ascii_case("#fff")
        || value.eq_ignore_ascii_case("#ffffff")
    {
        return false;
    }
    let hex = value.trim_start_matches('#');
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let number = u32::from_str_radix(hex, 16).unwrap_or(0);
        let red = (number >> 16) & 255;
        let green = (number >> 8) & 255;
        let blue = number & 255;
        return red < 250 || green < 250 || blue < 250;
    }
    true
}
