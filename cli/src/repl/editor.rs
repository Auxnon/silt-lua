//! reedline hooks: syntax highlighting, multi-line validation, completion and the
//! prompt, all driven by silt's own analysis (`silt_lua::lsp`).

use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use nu_ansi_term::Color;
use reedline::{
    CompletionResult, Completer, Highlighter, Prompt, PromptEditMode, PromptHistorySearch,
    PromptHistorySearchStatus, Span, StyledText, Suggestion, ValidationResult, Validator,
};
use silt_lua::lsp::{self, CompletionKind, Position};

use super::theme::Theme;

/// State the hooks share with the REPL loop: the theme (`:theme` swaps it), the
/// inputs run so far (completion sees their globals and functions), and the
/// buffer as last painted (auto-indent reads it on Enter).
#[derive(Clone)]
pub struct Shared {
    pub theme: Arc<Mutex<Theme>>,
    pub session: Arc<Mutex<String>>,
    pub buffer: Arc<Mutex<Mirror>>,
}

/// The line editor's buffer as auto-indent knows it. reedline parses a whole
/// batch of keys before applying any (and repaints after), so a line typed fast
/// and its Enter arrive together: the highlighter's copy is a batch behind, and
/// the edit mode replays the batch's text edits onto it. Any edit it can't
/// replay (motions, history, a menu) marks it inexact until the next repaint.
#[derive(Clone, Default)]
pub struct Mirror {
    pub text: String,
    pub cursor: usize,
    pub exact: bool,
}

impl Shared {
    pub fn new(theme: Theme) -> Shared {
        Shared {
            theme: Arc::new(Mutex::new(theme)),
            session: Arc::default(),
            buffer: Arc::default(),
        }
    }

    pub fn theme(&self) -> Theme {
        self.theme.lock().unwrap().clone()
    }

    /// The buffer and cursor, if known exactly.
    pub fn buffer(&self) -> Option<(String, usize)> {
        let m = self.buffer.lock().unwrap();
        m.exact.then(|| (m.text.clone(), m.cursor))
    }
}

/// Byte offset of char column `col` in `line` (clamped to its end).
fn byte_at(line: &str, col: u32) -> usize {
    line.char_indices()
        .nth(col as usize)
        .map_or(line.len(), |(i, _)| i)
}

pub struct SiltHighlighter(pub Shared);

impl Highlighter for SiltHighlighter {
    fn highlight(&self, line: &str, cursor: usize) -> StyledText {
        *self.0.buffer.lock().unwrap() = Mirror {
            text: line.to_string(),
            cursor,
            exact: true,
        };
        let theme = self.0.theme();
        let mut out = StyledText::new();
        if line.trim_start().starts_with(':') {
            out.push((theme.secondary(), line.to_string()));
            return out;
        }
        // absolute byte ranges of each token, in source order
        let starts: Vec<usize> = std::iter::once(0)
            .chain(line.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        let lines: Vec<&str> = line.split('\n').collect();
        let mut spans: Vec<(usize, usize, &str)> = lsp::semantic_tokens(line)
            .into_iter()
            .filter_map(|(l, col, len, kind)| {
                let text = lines.get(l as usize)?;
                let base = starts[l as usize];
                let from = base + byte_at(text, col);
                let to = base + byte_at(text, col + len);
                let kind = lsp::SEMANTIC_TOKEN_TYPES.get(kind as usize)?;
                (to > from).then_some((from, to, *kind))
            })
            .collect();
        spans.sort_by_key(|s| s.0);
        let mut at = 0;
        for (from, to, kind) in spans {
            if from < at {
                continue;
            }
            if from > at {
                out.push((theme.fg(), line[at..from].to_string()));
            }
            out.push((theme.token(kind), line[from..to].to_string()));
            at = to;
        }
        if at < line.len() {
            out.push((theme.fg(), line[at..].to_string()));
        }
        out
    }
}

/// Is `src` an unfinished chunk (an open block, bracket or expression), so Enter
/// should continue it on a new line rather than run it?
pub fn incomplete(src: &str) -> bool {
    // an open block or bracket (the parser reports `local t = {1,` as a missing
    // comma, not an early end)
    super::indent::depth(src) > 0
        || lsp::diagnostics(src).iter().any(|d| {
            let m = d.message.to_ascii_lowercase();
            (m.contains("unterminated") && !m.contains("string")) || m.contains("eof")
        })
}

/// Should Enter continue `src` on a new line? Not for a REPL command, nor on a
/// blank last line (so a mistyped block can still be run to see its error).
pub fn continues(src: &str) -> bool {
    let blank_tail = src.contains('\n') && src.rsplit('\n').next().unwrap_or("").trim().is_empty();
    !src.trim_start().starts_with(':') && !blank_tail && incomplete(src)
}

pub struct SiltValidator;

impl Validator for SiltValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if continues(line) {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

pub struct SiltCompleter(pub Shared);

impl Completer for SiltCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> CompletionResult {
        let before = &line[..pos];
        let word_start = before
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
            .last()
            .map_or(pos, |(i, _)| i);
        let prefix = &line[word_start..pos];

        // complete against everything run so far plus this input, so session
        // globals and functions are offered
        let session = self.0.session.lock().unwrap().clone();
        let source = format!("{}{}", session, line);
        let row = session.matches('\n').count() + before.matches('\n').count();
        let col = before.rsplit('\n').next().unwrap_or("").chars().count();
        let pos_in = Position { line: row as u32, character: col as u32 };

        let theme = self.0.theme();
        let mut seen = std::collections::HashSet::new();
        let suggestions: Vec<Suggestion> = lsp::completion(&source, pos_in)
            .into_iter()
            .filter(|c| c.label.starts_with(prefix) && seen.insert(c.label.clone()))
            .map(|c| {
                let style = match c.kind {
                    CompletionKind::Keyword => theme.token("keyword"),
                    CompletionKind::Function => theme.token("function"),
                    CompletionKind::Type => theme.token("type"),
                    _ => theme.fg(),
                };
                Suggestion {
                    value: c.label,
                    description: c.detail,
                    style: Some(style),
                    span: Span { start: word_start, end: pos },
                    append_whitespace: false,
                    ..Suggestion::default()
                }
            })
            .collect();
        CompletionResult::fresh(suggestions)
    }
}

/// `❯ `, and `┆ ` while a chunk continues: same width, so continued lines line up.
pub struct SiltPrompt(pub Shared);

impl Prompt for SiltPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("❯ ")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("┆ ")
    }
    fn render_prompt_history_search_indicator(&self, search: PromptHistorySearch) -> Cow<'_, str> {
        let failing = match search.status {
            PromptHistorySearchStatus::Passing => "",
            PromptHistorySearchStatus::Failing => "failing ",
        };
        Cow::Owned(format!("({}search: {}) ", failing, search.term))
    }
    fn get_indicator_color(&self) -> Color {
        let t = self.0.theme();
        if t.plain { Color::Default } else { t.palette.primary }
    }
    fn get_prompt_multiline_color(&self) -> Color {
        let t = self.0.theme();
        if t.plain { Color::Default } else { t.palette.faint }
    }
    fn get_prompt_color(&self) -> Color {
        let t = self.0.theme();
        if t.plain { Color::Default } else { t.palette.secondary }
    }
}
