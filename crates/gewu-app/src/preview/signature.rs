//! Top-level `#let name(...)` bindings in a Typst quote.

pub(super) struct Function {
    pub(super) name: String,
    pub(super) required: Vec<String>,
}

pub(super) fn functions_in(text: &str) -> Vec<Function> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut depth = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index = skip_string(bytes, index);
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if depth == 0
            && text.is_char_boundary(index)
            && text[index..].starts_with("#let ")
            && let Some((function, consumed)) = parse_function(&text[index..])
        {
            found.push(function);
            index += consumed;
            if found.len() == 4 {
                break;
            }
            continue;
        }
        match bytes[index] {
            b'{' => depth += 1,
            b'}' if depth > 0 => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    found
}

pub(super) fn skip_string(bytes: &[u8], open: usize) -> usize {
    let mut index = open + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn parse_function(text: &str) -> Option<(Function, usize)> {
    let rest = text.strip_prefix("#let ")?;
    let name = ident(rest)?;
    let after = &rest[name.len()..];
    let spaced = after.trim_start();
    if !spaced.starts_with('(') {
        return None;
    }
    let open = after.len() - spaced.len();
    let (args, span) = balanced(&after[open..])?;
    let required = split_args(args)
        .into_iter()
        .filter(|param| !param.contains(':'))
        .filter_map(|param| ident(param).map(str::to_owned))
        .collect();
    Some((
        Function {
            name: name.to_owned(),
            required,
        },
        "#let ".len() + name.len() + open + span,
    ))
}

fn ident(text: &str) -> Option<&str> {
    let text = text.trim();
    let end = text
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .unwrap_or(text.len());
    let name = &text[..end];
    name.starts_with(|ch: char| ch.is_ascii_alphabetic())
        .then_some(name)
}

fn balanced(text: &str) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index = skip_string(bytes, index);
            continue;
        }
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[1..index], index + 1));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn split_args(args: &str) -> Vec<&str> {
    let bytes = args.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            index = skip_string(bytes, index);
            continue;
        }
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b',' if depth == 0 => {
                parts.push(args[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    let tail = args[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}
