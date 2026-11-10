//! SVG path coordinates, enough to bound Typst's output.

pub(super) fn path_points(data: &str, points: &mut Vec<(f64, f64)>) {
    let bytes = data.as_bytes();
    let mut index = 0;
    let mut command = b'M';
    let mut x = 0.0;
    let mut y = 0.0;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        if bytes[index].is_ascii_alphabetic() {
            command = bytes[index];
            index += 1;
        }
        let count = match command {
            b'M' | b'm' | b'L' | b'l' | b'T' | b't' => 2,
            b'H' | b'h' | b'V' | b'v' => 1,
            b'S' | b's' | b'Q' | b'q' => 4,
            b'C' | b'c' => 6,
            b'A' | b'a' => 7,
            b'Z' | b'z' => 0,
            _ => break,
        };
        if count == 0 {
            continue;
        }
        let Some(args) = take_numbers(bytes, &mut index, count) else {
            break;
        };
        match command {
            b'M' | b'L' | b'T' => {
                x = args[0];
                y = args[1];
                points.push((x, y));
                if command == b'M' {
                    command = b'L';
                }
            }
            b'm' | b'l' | b't' => {
                x += args[0];
                y += args[1];
                points.push((x, y));
                if command == b'm' {
                    command = b'l';
                }
            }
            b'H' => {
                x = args[0];
                points.push((x, y));
            }
            b'h' => {
                x += args[0];
                points.push((x, y));
            }
            b'V' => {
                y = args[0];
                points.push((x, y));
            }
            b'v' => {
                y += args[0];
                points.push((x, y));
            }
            b'C' => {
                points.extend([(args[0], args[1]), (args[2], args[3]), (args[4], args[5])]);
                x = args[4];
                y = args[5];
            }
            b'c' => {
                points.extend([
                    (x + args[0], y + args[1]),
                    (x + args[2], y + args[3]),
                    (x + args[4], y + args[5]),
                ]);
                x += args[4];
                y += args[5];
            }
            b'S' | b'Q' => {
                points.extend([(args[0], args[1]), (args[2], args[3])]);
                x = args[2];
                y = args[3];
            }
            b's' | b'q' => {
                points.extend([(x + args[0], y + args[1]), (x + args[2], y + args[3])]);
                x += args[2];
                y += args[3];
            }
            b'A' => {
                let rx = args[0].abs();
                let ry = args[1].abs();
                x = args[5];
                y = args[6];
                points.extend([(x - rx, y - ry), (x + rx, y + ry)]);
            }
            b'a' => {
                let rx = args[0].abs();
                let ry = args[1].abs();
                x += args[5];
                y += args[6];
                points.extend([(x - rx, y - ry), (x + rx, y + ry)]);
            }
            _ => {}
        }
    }
}

fn take_numbers(bytes: &[u8], index: &mut usize, count: usize) -> Option<Vec<f64>> {
    let mut values = Vec::with_capacity(count);
    while values.len() < count {
        while *index < bytes.len() && (bytes[*index].is_ascii_whitespace() || bytes[*index] == b',')
        {
            *index += 1;
        }
        if *index < bytes.len() && bytes[*index].is_ascii_alphabetic() {
            return None;
        }
        let start = *index;
        if *index < bytes.len() && (bytes[*index] == b'-' || bytes[*index] == b'+') {
            *index += 1;
        }
        let mut saw_digit = false;
        while *index < bytes.len() && bytes[*index].is_ascii_digit() {
            saw_digit = true;
            *index += 1;
        }
        if *index < bytes.len() && bytes[*index] == b'.' {
            *index += 1;
            while *index < bytes.len() && bytes[*index].is_ascii_digit() {
                saw_digit = true;
                *index += 1;
            }
        }
        if !saw_digit {
            return None;
        }
        values.push(
            std::str::from_utf8(&bytes[start..*index])
                .ok()?
                .parse()
                .ok()?,
        );
    }
    Some(values)
}

pub(super) fn numbers_in(text: &str) -> impl Iterator<Item = f64> + '_ {
    let bytes = text.as_bytes();
    let mut index = 0;
    std::iter::from_fn(move || {
        while index < bytes.len()
            && !bytes[index].is_ascii_digit()
            && bytes[index] != b'-'
            && bytes[index] != b'.'
            && bytes[index] != b'+'
        {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        let start = index;
        if bytes[index] == b'-' || bytes[index] == b'+' {
            index += 1;
        }
        let mut saw_digit = false;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            saw_digit = true;
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                saw_digit = true;
                index += 1;
            }
        }
        if !saw_digit {
            return None;
        }
        text[start..index].parse().ok()
    })
}

pub(super) fn tag_name(svg: &str, at: usize) -> &str {
    let bytes = svg.as_bytes();
    let mut end = at;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b':') {
        end += 1;
    }
    svg.get(at..end).unwrap_or("")
}
