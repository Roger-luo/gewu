use super::{markdown::Markdown, style::Document};

pub struct HelpScreen;

impl HelpScreen {
    #[must_use]
    pub fn document() -> Document {
        Markdown::render(
            "# Review help\n\n\
             ## Feedback\n\n\
             - **`c` Comment** — records a non-blocking observation on the current chunk.\n\
             - **`!` Request change** — records a concern that remains unresolved until explicitly resolved. Say what is wrong: the code, the code at one line, or this chunk's explanation of it. An agent fixes those in different places.\n\
             - **`Shift`+`Enter`** — adds a line to what you are writing; **`Enter`** saves it. Pasted text keeps its own line breaks.\n\n\
             ## While writing\n\n\
             - **`←` / `→` / `↑` / `↓`** — move the cursor; **`Alt`** with `←` / `→` moves by word.\n\
             - **`Home` / `End`** or **`Ctrl`+`a` / `Ctrl`+`e`** — start or end of the line.\n\
             - **`Backspace` / `Delete`** — erase either side of the cursor; **`Ctrl`+`w`** erases a word, **`Ctrl`+`u`** and **`Ctrl`+`k`** the line before and after it.\n\
             - **`PageUp` / `PageDown`** — scroll the chunk you are commenting on, without leaving what you have written.\n\n\
             ## Selected feedback\n\n\
             - **`f` Select** — picks out the feedback on the current chunk; **`↑` / `↓`** moves between items and **`Esc`** leaves the selection.\n\
             - **`x` Resolve** — resolves the selected request, or reopens it.\n\
             - **`t` Re-aim** — moves the selected feedback between the code and this chunk's explanation, for when it was filed at the wrong one.\n\
             - **`d` Delete** — withdraws the selected feedback after you confirm, for when you left it by mistake.\n\n\
             ## Ending the review\n\n\
             - The outcome follows what you left: a request open means **request changes**, remarks alone mean **comment**, nothing at all means **approve**. There is no separate button to press, and nothing to keep in step with your feedback.\n\
             - **`Enter`** on the summary ends the review with that outcome. If part of the change set has not been written into the review yet, you are asked first, because ending it stops the rest being written.\n\
             - **`u`** jumps to the first unreviewed chunk; **`g`** to the first open request.\n\n\
             ## Navigation\n\n\
             - **`↑` / `↓` or `k` / `j`** — scroll one line.\n\
             - **`PageUp` / `PageDown`** — scroll one page.\n\
             - **`Enter`** — done with this chunk: marks it reviewed and moves on. A chunk you left feedback on is not claimed this way; say what you meant with `r`.\n\
             - **`←` / `→` or `p` / `n`** — move between chunks without marking anything, for reading ahead.\n\
             - **`r`** — mark the current chunk reviewed, or unmark it.\n\
             - **`q`** — quit and preserve progress.\n\n\
             Press **`Esc`** or **`?`** to close this help.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::HelpScreen;

    #[test]
    fn explains_feedback_actions_and_escape() {
        let text: String = HelpScreen::document()
            .lines
            .iter()
            .flat_map(|line| &line.spans)
            .map(|span| span.text.as_str())
            .collect();

        assert!(text.contains("Comment"));
        assert!(text.contains("Request change"));
        assert!(text.contains("Esc"));
    }
}
