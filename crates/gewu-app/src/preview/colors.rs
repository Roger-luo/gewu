/// A color a reviewer can see, named when the line binds it (`#let ink = ...`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Swatch {
    pub(crate) name: Option<String>,
    pub(crate) hex: String,
}

/// Typst `rgb(...)` and `luma(...)` literals in `text`, in order.
pub(crate) fn swatches_in(text: &str) -> Vec<Swatch> {
    let mut found = Vec::new();
    for line in text.lines() {
        let name = binding_name(line);
        let colors = colors_in(line);
        if colors.len() == 1 && name.is_some() {
            found.push(Swatch {
                name,
                hex: colors[0].clone(),
            });
        } else {
            found.extend(colors.into_iter().map(|hex| Swatch { name: None, hex }));
        }
    }
    found
}

/// `#let ink = ...` → `ink`.
fn binding_name(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix("#let ")?.trim_start();
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect();
    (!name.is_empty()).then_some(name)
}

fn colors_in(line: &str) -> Vec<String> {
    let mut colors = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let rgb_at = rest.find("rgb(");
        let luma_at = rest.find("luma(");
        let at = match (rgb_at, luma_at) {
            (Some(rgb), Some(luma)) => rgb.min(luma),
            (Some(rgb), None) => rgb,
            (None, Some(luma)) => luma,
            (None, None) => break,
        };
        rest = &rest[at..];
        if let Some(args) = call_at(rest, 0, "rgb(")
            && let Some(hex) = rgb_args(args)
        {
            colors.push(hex);
        } else if let Some(args) = call_at(rest, 0, "luma(")
            && let Some(hex) = luma_args(args)
        {
            colors.push(hex);
        }
        rest = rest.get(1..).unwrap_or("");
    }
    colors
}

fn call_at<'a>(line: &'a str, index: usize, name: &str) -> Option<&'a str> {
    let rest = line.get(index..)?;
    let args = rest.strip_prefix(name)?;
    let end = args.find(')')?;
    Some(&args[..end])
}

fn rgb_args(args: &str) -> Option<String> {
    let args = args.trim();
    if let Some(quoted) = quoted(args) {
        return hex_digits(quoted);
    }
    let mut parts = args.split(',').map(str::trim);
    let red = channel(parts.next()?)?;
    let green = channel(parts.next()?)?;
    let blue = channel(parts.next()?)?;
    Some(format!("#{red:02x}{green:02x}{blue:02x}"))
}

fn luma_args(args: &str) -> Option<String> {
    let level = channel(args.split(',').next()?.trim())?;
    Some(format!("#{level:02x}{level:02x}{level:02x}"))
}

fn quoted(args: &str) -> Option<&str> {
    let args = args.trim();
    let body = args.strip_prefix('"')?.strip_suffix('"')?;
    Some(body.trim().trim_start_matches('#'))
}

fn hex_digits(digits: &str) -> Option<String> {
    let digits = digits.trim_start_matches('#');
    if digits.len() < 6 || !digits.as_bytes()[..6].iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    Some(format!("#{}", digits[..6].to_ascii_lowercase()))
}

/// A `0-255` channel, a `0-1` fraction, or a percentage of 255.
fn channel(text: &str) -> Option<u8> {
    let text = text.trim();
    let percent = text.ends_with('%');
    let number: f32 = text.trim_end_matches('%').trim().parse().ok()?;
    let value = if percent {
        number / 100.0 * 255.0
    } else if text.contains('.') && (0.0..=1.0).contains(&number) {
        number * 255.0
    } else {
        number
    };
    Some(value.round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::swatches_in;

    #[test]
    fn a_typst_palette_names_each_bound_color() {
        let text = "\
#let ink = rgb(\"243746\")
#let muted = rgb(\"#526575\")
#let quiet = rgb(\"D2DCE3\")
#let accent = rgb(\"0072B2\")
#let data = rgb(\"009E73\") // Okabe–Ito bluish green
#let feedback = rgb(\"B86A00\")
#let paper = luma(96%)
#let shade = rgb(14%, 22%, 28%)
";
        let swatches = swatches_in(text);
        assert_eq!(swatches[0].name.as_deref(), Some("ink"));
        assert_eq!(swatches[0].hex, "#243746");
        assert_eq!(swatches[1].hex, "#526575");
        assert_eq!(swatches[2].hex, "#d2dce3");
        assert_eq!(swatches[5].name.as_deref(), Some("feedback"));
        assert_eq!(swatches[5].hex, "#b86a00");
        assert_eq!(swatches[6].hex, "#f5f5f5");
        assert_eq!(swatches[7].name.as_deref(), Some("shade"));
    }
}
