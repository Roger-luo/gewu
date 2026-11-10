//! Shape bounds, including transforms, for the viewport fit.

use super::path;
use super::{Bounds, Matrix, attr};

/// Bounds of every shape except a page-colored fill.
pub(super) fn measure_ink(svg: &str) -> Option<Bounds> {
    walk(svg, true)
}

/// Remove a near-white rectangle that is the page, when the drawing continues
/// past it. A white mark that is not the page stays.
pub(super) fn strip_page_fill(svg: &str, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> String {
    let view_area = (max_x - min_x) * (max_y - min_y);
    let bytes = svg.as_bytes();
    let mut index = 0;
    let mut stack = vec![Matrix::IDENTITY];
    let mut skip = 0i32;
    let mut drop_at = Vec::new();
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        if bytes.get(index + 1) == Some(&b'/') {
            let name = path::tag_name(svg, index + 2);
            if matches!(name, "defs" | "clipPath" | "mask" | "pattern" | "symbol") && skip > 0 {
                skip -= 1;
            }
            if stack.len() > 1 {
                stack.pop();
            }
            index = svg[index..]
                .find('>')
                .map_or(svg.len(), |at| index + at + 1);
            continue;
        }
        if bytes.get(index + 1) == Some(&b'!') {
            index += 1;
            continue;
        }
        let name = path::tag_name(svg, index + 1);
        let close = svg[index..].find('>').map_or(svg.len(), |at| index + at);
        let tag = &svg[index..=close];
        let parent = stack.last().copied().unwrap_or(Matrix::IDENTITY);
        let matrix = parent * transform(attr(tag, "transform").unwrap_or(""));
        let self_closing = tag.ends_with("/>");
        if matches!(name, "defs" | "clipPath" | "mask" | "pattern" | "symbol") && !self_closing {
            skip += 1;
        } else if skip == 0
            && self_closing
            && paper_fill(tag)
            && shape_bounds(matrix, name, tag).is_some_and(|bounds| {
                let area = (bounds.max_x - bounds.min_x) * (bounds.max_y - bounds.min_y);
                area >= view_area * 0.5
                    && bounds.min_x >= min_x - 1.0
                    && bounds.min_y >= min_y - 1.0
                    && bounds.max_x <= max_x + 1.0
                    && bounds.max_y <= max_y + 1.0
            })
        {
            drop_at.push((index, close + 1));
        }
        if !self_closing {
            stack.push(matrix);
        }
        index = close + 1;
    }
    if drop_at.is_empty() {
        return svg.to_owned();
    }
    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0;
    for (start, end) in drop_at {
        out.push_str(&svg[cursor..start]);
        cursor = end;
    }
    out.push_str(&svg[cursor..]);
    out
}

fn walk(svg: &str, skip_paper: bool) -> Option<Bounds> {
    let bytes = svg.as_bytes();
    let mut index = 0;
    let mut stack = vec![Matrix::IDENTITY];
    let mut skip = 0i32;
    let mut bounds: Option<Bounds> = None;
    while index < bytes.len() {
        if bytes[index] != b'<' {
            index += 1;
            continue;
        }
        if bytes.get(index + 1) == Some(&b'/') {
            let name = path::tag_name(svg, index + 2);
            if matches!(name, "defs" | "clipPath" | "mask" | "pattern" | "symbol") && skip > 0 {
                skip -= 1;
            }
            if stack.len() > 1 {
                stack.pop();
            }
            index = svg[index..]
                .find('>')
                .map_or(svg.len(), |at| index + at + 1);
            continue;
        }
        if bytes.get(index + 1) == Some(&b'!') {
            index += 1;
            continue;
        }
        let name = path::tag_name(svg, index + 1);
        let close = svg[index..].find('>').map_or(svg.len(), |at| index + at);
        let tag = &svg[index..=close];
        let parent = stack.last().copied().unwrap_or(Matrix::IDENTITY);
        let matrix = parent * transform(attr(tag, "transform").unwrap_or(""));
        let self_closing = tag.ends_with("/>");
        if matches!(name, "defs" | "clipPath" | "mask" | "pattern" | "symbol") && !self_closing {
            skip += 1;
        } else if skip == 0
            && !(skip_paper && paper_fill(tag))
            && let Some(shape) = shape_bounds(matrix, name, tag)
        {
            let bounds = bounds.get_or_insert(shape);
            bounds.include(shape.min_x, shape.min_y);
            bounds.include(shape.max_x, shape.max_y);
        }
        if !self_closing {
            stack.push(matrix);
        }
        index = close + 1;
    }
    bounds
}

fn paper_fill(tag: &str) -> bool {
    attr(tag, "fill").is_some_and(super::super::paint::is_paper)
}

fn shape_bounds(matrix: Matrix, name: &str, tag: &str) -> Option<Bounds> {
    let mut points = Vec::new();
    match name {
        "path" => {
            if let Some(data) = attr(tag, "d") {
                path::path_points(data, &mut points);
            }
        }
        "rect" => rect_points(tag, &mut points),
        "circle" | "ellipse" => ellipse_points(tag, &mut points),
        "line" => {
            push_attr_point(tag, "x1", "y1", &mut points);
            push_attr_point(tag, "x2", "y2", &mut points);
        }
        "polyline" | "polygon" => {
            if let Some(raw) = attr(tag, "points") {
                let nums: Vec<f64> = path::numbers_in(raw).collect();
                for [x, y] in nums.as_chunks::<2>().0 {
                    points.push((*x, *y));
                }
            }
        }
        _ => {}
    }
    let mut bounds = None;
    for (x, y) in points {
        let (x, y) = matrix.apply(x, y);
        let bounds = bounds.get_or_insert(Bounds {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        });
        bounds.include(x, y);
    }
    bounds
}

fn rect_points(tag: &str, points: &mut Vec<(f64, f64)>) {
    let x = attr_f64(tag, "x").unwrap_or(0.0);
    let y = attr_f64(tag, "y").unwrap_or(0.0);
    let width = attr_f64(tag, "width").unwrap_or(0.0);
    let height = attr_f64(tag, "height").unwrap_or(0.0);
    points.extend([
        (x, y),
        (x + width, y),
        (x, y + height),
        (x + width, y + height),
    ]);
}

fn ellipse_points(tag: &str, points: &mut Vec<(f64, f64)>) {
    let cx = attr_f64(tag, "cx").unwrap_or(0.0);
    let cy = attr_f64(tag, "cy").unwrap_or(0.0);
    let rx = attr_f64(tag, "r")
        .or_else(|| attr_f64(tag, "rx"))
        .unwrap_or(0.0);
    let ry = attr_f64(tag, "r")
        .or_else(|| attr_f64(tag, "ry"))
        .unwrap_or(rx);
    points.extend([(cx - rx, cy - ry), (cx + rx, cy + ry)]);
}

fn push_attr_point(tag: &str, x: &str, y: &str, points: &mut Vec<(f64, f64)>) {
    if let (Some(x), Some(y)) = (attr_f64(tag, x), attr_f64(tag, y)) {
        points.push((x, y));
    }
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    path::numbers_in(attr(tag, name)?).next()
}

fn transform(value: &str) -> Matrix {
    let mut combined = Matrix::IDENTITY;
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_alphabetic() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
                index += 1;
            }
            let name = &value[start..index];
            let Some(open) = value[index..].find('(') else {
                break;
            };
            let args_at = index + open + 1;
            let Some(close) = value[args_at..].find(')') else {
                break;
            };
            let args: Vec<f64> = path::numbers_in(&value[args_at..args_at + close]).collect();
            let next = match name {
                "matrix" if args.len() >= 6 => Matrix {
                    a: args[0],
                    b: args[1],
                    c: args[2],
                    d: args[3],
                    e: args[4],
                    f: args[5],
                },
                "translate" if args.len() >= 2 => Matrix {
                    e: args[0],
                    f: args[1],
                    ..Matrix::IDENTITY
                },
                "translate" if args.len() == 1 => Matrix {
                    e: args[0],
                    ..Matrix::IDENTITY
                },
                "scale" if args.len() >= 2 => Matrix {
                    a: args[0],
                    d: args[1],
                    ..Matrix::IDENTITY
                },
                "scale" if args.len() == 1 => Matrix {
                    a: args[0],
                    d: args[0],
                    ..Matrix::IDENTITY
                },
                _ => Matrix::IDENTITY,
            };
            combined = combined * next;
            index = args_at + close + 1;
        } else {
            index += 1;
        }
    }
    combined
}
