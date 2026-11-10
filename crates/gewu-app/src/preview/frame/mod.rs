//! Grow an SVG viewport so ink drawn outside it stays on screen.
//!
//! Typst sizes an automatic page to the flow, then places figures past that
//! edge. Those marks are in the file, and the viewport cuts them off.

use std::ops::Mul;

mod geometry;
mod path;

use path::numbers_in;

#[derive(Clone, Copy)]
pub(super) struct Matrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Matrix {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

impl Mul for Matrix {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    fn include(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

#[derive(Clone, Copy)]
struct View {
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
}

impl View {
    fn covers(self, bounds: Bounds) -> bool {
        bounds.min_x >= self.min_x - 0.5
            && bounds.min_y >= self.min_y - 0.5
            && bounds.max_x <= self.min_x + self.width + 0.5
            && bounds.max_y <= self.min_y + self.height + 0.5
    }
}

/// Expand `viewBox` and the root width and height until every shape fits,
/// with a small pad. A picture that already fits is unchanged.
pub(super) fn fit(svg: &str) -> String {
    let Some(view) = view_box(svg) else {
        return svg.to_owned();
    };
    let Some(ink) = geometry::measure_ink(svg) else {
        return svg.to_owned();
    };
    // The white rectangle is the page, not the drawing. When marks are placed
    // past it, that patch is smaller than the figure and reads as a hole in
    // dark mode. Drop it and frame the marks.
    let page_was_short = !view.covers(ink);
    let svg = if page_was_short {
        geometry::strip_page_fill(
            svg,
            view.min_x,
            view.min_y,
            view.min_x + view.width,
            view.min_y + view.height,
        )
    } else {
        svg.to_owned()
    };
    let Some(view) = view_box(&svg) else {
        return svg;
    };
    let Some(bounds) = geometry::measure_ink(&svg) else {
        return svg;
    };
    if view.covers(bounds) {
        return svg;
    }
    let pad = 6.0;
    let min_x = if page_was_short {
        bounds.min_x - pad
    } else {
        view.min_x.min(bounds.min_x - pad)
    };
    let min_y = if page_was_short {
        bounds.min_y - pad
    } else {
        view.min_y.min(bounds.min_y - pad)
    };
    let max_x = if page_was_short {
        bounds.max_x + pad
    } else {
        (view.min_x + view.width).max(bounds.max_x + pad)
    };
    let max_y = if page_was_short {
        bounds.max_y + pad
    } else {
        (view.min_y + view.height).max(bounds.max_y + pad)
    };
    let width = max_x - min_x;
    let height = max_y - min_y;
    let unit = size_unit(&svg);
    rewrite_root(
        &svg,
        &format!(
            "{} {} {} {}",
            num(min_x),
            num(min_y),
            num(width),
            num(height)
        ),
        &format!("{}{unit}", num(width)),
        &format!("{}{unit}", num(height)),
    )
}

fn num(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let mut text = format!("{rounded:.2}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

fn view_box(svg: &str) -> Option<View> {
    let tag = root_tag(svg)?;
    let raw = attr(&tag, "viewBox")?;
    let mut numbers = numbers_in(raw);
    let min_x = numbers.next()?;
    let min_y = numbers.next()?;
    let width = numbers.next()?;
    let height = numbers.next()?;
    (width > 0.0 && height > 0.0).then_some(View {
        min_x,
        min_y,
        width,
        height,
    })
}

fn size_unit(svg: &str) -> &'static str {
    let Some(tag) = root_tag(svg) else {
        return "";
    };
    let Some(width) = attr(&tag, "width") else {
        return "";
    };
    if width.ends_with("pt") {
        "pt"
    } else if width.ends_with("px") {
        "px"
    } else {
        ""
    }
}

fn root_tag(svg: &str) -> Option<String> {
    let start = svg.find("<svg")?;
    let end = svg[start..].find('>')? + start;
    Some(svg[start..=end].to_owned())
}

pub(super) fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn rewrite_root(svg: &str, view_box: &str, width: &str, height: &str) -> String {
    let Some(start) = svg.find("<svg") else {
        return svg.to_owned();
    };
    let Some(relative) = svg[start..].find('>') else {
        return svg.to_owned();
    };
    let end = start + relative;
    let mut tag = svg[start..end].to_owned();
    tag = set_attr(&tag, "viewBox", view_box);
    tag = set_attr(&tag, "width", width);
    tag = set_attr(&tag, "height", height);
    let mut out = String::with_capacity(svg.len() + tag.len());
    out.push_str(&svg[..start]);
    out.push_str(&tag);
    out.push_str(&svg[end..]);
    out
}

fn set_attr(tag: &str, name: &str, value: &str) -> String {
    let needle = format!("{name}=\"");
    if let Some(index) = tag.find(&needle) {
        let value_at = index + needle.len();
        let Some(end) = tag[value_at..].find('"') else {
            return tag.to_owned();
        };
        let mut out = String::new();
        out.push_str(&tag[..value_at]);
        out.push_str(value);
        out.push_str(&tag[value_at + end..]);
        return out;
    }
    let mut out = String::new();
    out.push_str(tag);
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(value);
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::fit;

    #[test]
    fn a_placed_mark_outside_the_page_stays_in_the_viewport() {
        let svg = "\
<svg viewBox=\"0 0 24 24\" width=\"24pt\" height=\"24pt\">\
<path fill=\"#ffffff\" d=\"M 0 0 h 24 v 24 h -24 Z\"/>\
<path fill=\"#0072b2\" transform=\"matrix(1 0 0 1 49 37)\" d=\"M 0 0 m 0 3.5 c 0 -1.93 1.57 -3.5 3.5 -3.5\"/>\
</svg>";
        let fitted = fit(svg);
        let tag_end = fitted.find('>').unwrap();
        let tag = &fitted[..tag_end];
        assert!(
            tag.contains("viewBox=\"0 0 64.5 52.5\"") || view_holds(&fitted, 52.5, 40.5),
            "{tag}"
        );
        assert!(tag.contains("width="), "{tag}");
        assert!(
            !fitted.to_ascii_lowercase().contains("ffffff"),
            "page fill still covers only part of the drawing: {fitted}"
        );
    }

    #[test]
    fn a_drawing_inside_its_page_is_left_alone() {
        let svg = "<svg viewBox=\"0 0 80 40\" width=\"80pt\" height=\"40pt\"><path d=\"M 8 8 L 40 20\"/></svg>";
        assert_eq!(fit(svg), svg);
    }

    fn view_holds(svg: &str, x: f64, y: f64) -> bool {
        let start = svg.find("viewBox=\"").unwrap() + "viewBox=\"".len();
        let end = svg[start..].find('"').unwrap() + start;
        let nums: Vec<f64> = super::path::numbers_in(&svg[start..end]).collect();
        nums.len() == 4
            && x >= nums[0]
            && y >= nums[1]
            && x <= nums[0] + nums[2]
            && y <= nums[1] + nums[3]
    }
}
