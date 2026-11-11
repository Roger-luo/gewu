//! Reading test code line by line: where each test starts, what it is
//! called, and what its doc comment says. Hunk lines carry their diff marker
//! (`+`, `-`, or a space), which is stripped before looking.

/// The code of a hunk body line, without its diff marker.
pub(super) fn code(line: &str) -> &str {
    line.get(1..).unwrap_or("")
}

/// Whether `line` opens a test: a test attribute, or a test definition in a
/// language that marks tests by name or call.
pub(super) fn starts_test(line: &str) -> bool {
    let text = code(line).trim_start();
    (text.starts_with("#[") && text.contains("test"))
        || text.starts_with("def test")
        || text.starts_with("async def test")
        || text.starts_with("func Test")
        || text.starts_with("@Test")
        || ["it(", "test(", "it.each", "test.each"]
            .iter()
            .any(|call| text.starts_with(call))
}

/// Whether `line` belongs above the test it precedes: a doc comment, a
/// comment, or another attribute.
pub(super) fn leads_test(line: &str) -> bool {
    let text = code(line).trim_start();
    text.starts_with("//") || text.starts_with("#[") || text.starts_with('@')
}

/// The name a test is defined with, from the first line of `lines` that
/// defines one.
pub(super) fn test_name(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|line| {
        let text = code(line).trim_start();
        let text = text
            .strip_prefix("pub ")
            .unwrap_or(text)
            .trim_start_matches("async ");
        for keyword in ["fn ", "def ", "func "] {
            if let Some(rest) = text.strip_prefix(keyword) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
        for call in ["it(", "test("] {
            if let Some(rest) = text.strip_prefix(call) {
                let quote = rest
                    .chars()
                    .next()
                    .filter(|c| matches!(c, '"' | '\'' | '`'))?;
                let name: String = rest[1..].chars().take_while(|c| *c != quote).collect();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
        None
    })
}

/// The doc comment (`///`) on the test `lines` define, as one paragraph.
pub(super) fn doc(lines: &[&str]) -> Option<String> {
    let text: Vec<&str> = lines
        .iter()
        .map(|line| code(line).trim_start())
        .take_while(|text| !text.starts_with("fn ") && !text.starts_with("async fn "))
        .filter_map(|text| text.strip_prefix("///"))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect();
    (!text.is_empty()).then(|| text.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{doc, starts_test, test_name};

    #[test]
    fn finds_rust_python_and_javascript_tests() {
        assert!(starts_test("+#[test]"));
        assert!(starts_test("+    #[tokio::test]"));
        assert!(starts_test(" def test_rollback(self):"));
        assert!(starts_test("+  it(\"keeps the tail\", () => {"));
        assert!(!starts_test("+fn helper() {}"));

        let rust = [
            "+/// Grows by a quarter.",
            "+#[test]",
            "+fn grows_by_a_quarter() {",
        ];
        assert_eq!(test_name(&rust).as_deref(), Some("grows_by_a_quarter"));
        assert_eq!(doc(&rust).as_deref(), Some("Grows by a quarter."));
        let js = ["+  it('keeps the tail', () => {"];
        assert_eq!(test_name(&js).as_deref(), Some("keeps the tail"));
    }
}
