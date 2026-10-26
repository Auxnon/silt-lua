//! Auto-indent: Enter in an unfinished chunk starts the next line one level in per
//! open block or bracket, and a line led by `end`, `}`, `else`, … steps back out.
//! reedline has no hook for this (its validator can only say "continue"), so
//! `SiltEditMode` wraps the emacs bindings and decides Enter and Tab itself
//! against its `Mirror` of the buffer.

use reedline::{EditCommand, EditMode, Emacs, PromptEditMode, ReedlineEvent, ReedlineRawEvent};

use super::editor::{continues, Mirror, Shared};

pub const UNIT: &str = "  ";
/// Sentinels the keybindings send for Enter and Tab; `SiltEditMode` swaps them
/// for real events and they never reach reedline.
pub const ENTER: &str = "silt:enter";
pub const TAB: &str = "silt:tab";

/// Net blocks and brackets left open by `src`, skipping strings and comments.
pub fn depth(src: &str) -> usize {
    let mut d: isize = 0;
    scan(src, |w| d += weight(w));
    d.max(0) as usize
}

/// How a word or bracket changes the depth. `if`, `while` and `for` open
/// nothing themselves: their `then` / `do` does. `elseif` closes the branch its
/// own `then` reopens.
fn weight(w: &str) -> isize {
    match w {
        "function" | "do" | "then" | "repeat" | "{" | "(" | "[" => 1,
        "end" | "until" | "elseif" | "}" | ")" | "]" => -1,
        _ => 0,
    }
}

/// Does `line` start by closing (or switching) a block, so it sits one level out?
fn leads_out(line: &str) -> bool {
    let mut first = None;
    scan(line, |w| {
        first.get_or_insert_with(|| w.to_string());
    });
    matches!(
        first.as_deref(),
        Some("end" | "until" | "else" | "elseif" | "}" | ")" | "]")
    )
}

/// Feed `f` each word and bracket of `src`, outside strings and comments.
fn scan(src: &str, mut f: impl FnMut(&str)) {
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            i += 2;
            match long_bracket(b, i) {
                Some(level) => i = close_long(b, i, level),
                None => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
            }
        } else if c == b'"' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c && b[i] != b'\n' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if let Some(level) = (c == b'[').then(|| long_bracket(b, i)).flatten() {
            i = close_long(b, i, level);
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            f(&src[start..i]);
        } else {
            if matches!(c, b'{' | b'}' | b'(' | b')' | b'[' | b']') {
                f(&src[i..i + 1]);
            }
            i += 1;
        }
    }
}

/// The level of a `[[` / `[=[` long bracket opening at `i`.
fn long_bracket(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let level = b[i + 1..].iter().take_while(|&&c| c == b'=').count();
    (b.get(i + 1 + level) == Some(&b'[')).then_some(level)
}

/// Just past the `]=]` closing a long bracket of `level` opened at `i` (or the end).
fn close_long(b: &[u8], i: usize, level: usize) -> usize {
    let mut j = i + level + 2;
    while j < b.len() {
        if b[j] == b']'
            && b[j + 1..].iter().take(level).all(|&c| c == b'=')
            && b.get(j + 1 + level) == Some(&b']')
        {
            return j + level + 2;
        }
        j += 1;
    }
    b.len()
}

/// What Enter does with `buf` and the cursor at byte `cursor`: continue an
/// unfinished chunk on an indented line, or run it, re-indenting the line the
/// cursor is on first when it leads with `end` and friends.
pub fn enter(buf: &str, cursor: usize) -> ReedlineEvent {
    let line_start = buf[..cursor].rfind('\n').map_or(0, |i| i + 1);
    let mut fix = Vec::new();
    let mut before = buf[..cursor].to_string();
    // only the line being finished, with the cursor at its end, is re-indented
    let line_end = buf[cursor..].find('\n').map_or(buf.len(), |i| cursor + i);
    if line_start > 0 && line_end == cursor {
        let line = &buf[line_start..cursor];
        let lead = line.len() - line.trim_start().len();
        let want = UNIT.repeat(depth(&buf[..line_start]).saturating_sub(leads_out(line) as usize));
        if line[..lead] != want && !line.trim().is_empty() {
            fix.push(EditCommand::MoveToPosition { position: line_start, select: false });
            fix.extend(std::iter::repeat_n(EditCommand::Delete, line[..lead].chars().count()));
            fix.push(EditCommand::InsertString(want.clone()));
            fix.push(EditCommand::MoveToPosition {
                position: line_start + want.len() + line.len() - lead,
                select: false,
            });
            before = format!("{}{}{}", &buf[..line_start], want, line.trim_start());
        }
    }
    let after = &buf[cursor..];
    if continues(&format!("{}{}", before, after)) {
        fix.push(EditCommand::InsertNewline);
        fix.push(EditCommand::InsertString(UNIT.repeat(depth(&before))));
        ReedlineEvent::Edit(fix)
    } else if fix.is_empty() {
        ReedlineEvent::Enter
    } else {
        ReedlineEvent::Multiple(vec![ReedlineEvent::Edit(fix), ReedlineEvent::Enter])
    }
}

/// Replay `cmds` onto the mirror; anything but plain text edits makes it inexact.
fn replay(m: &mut Mirror, cmds: &[EditCommand]) {
    for cmd in cmds {
        if !m.exact {
            return;
        }
        match cmd {
            EditCommand::InsertChar(c) => {
                m.text.insert(m.cursor, *c);
                m.cursor += c.len_utf8();
            }
            EditCommand::InsertString(s) => {
                m.text.insert_str(m.cursor, s);
                m.cursor += s.len();
            }
            EditCommand::InsertNewline => {
                m.text.insert(m.cursor, '\n');
                m.cursor += 1;
            }
            EditCommand::Backspace => {
                if let Some(c) = m.text[..m.cursor].chars().next_back() {
                    m.cursor -= c.len_utf8();
                    m.text.remove(m.cursor);
                }
            }
            EditCommand::Delete => {
                if m.cursor < m.text.len() {
                    m.text.remove(m.cursor);
                }
            }
            EditCommand::MoveToPosition { position, select: false } if *position <= m.text.len() => {
                m.cursor = *position;
            }
            _ => m.exact = false,
        }
    }
}

/// emacs bindings, with Enter and Tab decided against the current buffer.
pub struct SiltEditMode {
    pub inner: Emacs,
    pub shared: Shared,
}

impl EditMode for SiltEditMode {
    fn parse_event(&mut self, event: ReedlineRawEvent) -> ReedlineEvent {
        let buffer = self.shared.buffer();
        let event = match self.inner.parse_event(event) {
            ReedlineEvent::ExecuteHostCommand(cmd) if cmd == ENTER => match buffer {
                // a menu takes Enter to accept its selection
                Some((buf, cursor)) => {
                    ReedlineEvent::UntilFound(vec![ReedlineEvent::MenuAccept, enter(&buf, cursor)])
                }
                // unsure of the buffer: a plain Enter (the validator still continues)
                None => ReedlineEvent::Enter,
            },
            ReedlineEvent::ExecuteHostCommand(cmd) if cmd == TAB => {
                // nothing to complete on a blank continuation line: indent instead
                let blank = buffer.is_some_and(|(buf, cursor)| {
                    cursor > 0 && buf[buf[..cursor].rfind('\n').map_or(0, |i| i + 1)..cursor].trim().is_empty()
                });
                if blank {
                    ReedlineEvent::Edit(vec![EditCommand::InsertString(UNIT.into())])
                } else {
                    ReedlineEvent::UntilFound(vec![
                        ReedlineEvent::Menu("completion_menu".into()),
                        ReedlineEvent::MenuNext,
                    ])
                }
            }
            other => other,
        };
        let mut m = self.shared.buffer.lock().unwrap();
        match &event {
            ReedlineEvent::Edit(cmds) => replay(&mut m, cmds),
            ReedlineEvent::None => {}
            // submits, menus, motions, history: unknown until the next repaint
            _ => m.exact = false,
        }
        event
    }

    fn edit_mode(&self) -> PromptEditMode {
        self.inner.edit_mode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_counts_blocks_and_brackets() {
        assert_eq!(depth("local function f()"), 1);
        assert_eq!(depth("local function f()\n  local function g(x)"), 2);
        assert_eq!(depth("if x then\n  y()\nelseif z then"), 1);
        assert_eq!(depth("while x do"), 1);
        assert_eq!(depth("local t = {\n  a = {"), 2);
        assert_eq!(depth("f() end end"), 0);
    }

    #[test]
    fn depth_skips_strings_and_comments() {
        assert_eq!(depth("local s = \"function (\" -- do then {"), 0);
        assert_eq!(depth("local s = [[\nfunction\n]] do"), 1);
        assert_eq!(depth("--[==[ function ]] do ]==] do"), 1);
    }

    #[test]
    fn closers_lead_out() {
        assert!(leads_out("  end"));
        assert!(leads_out("else"));
        assert!(leads_out("}, x"));
        assert!(!leads_out("  ending = 1"));
        assert!(!leads_out("  -- end"));
    }

    #[test]
    fn enter_indents_a_nested_function() {
        let buf = "local function outer()\n  local function inner(x)";
        match enter(buf, buf.len()) {
            ReedlineEvent::Edit(cmds) => {
                assert_eq!(cmds.last(), Some(&EditCommand::InsertString("    ".into())))
            }
            e => panic!("{:?}", e),
        }
    }

    #[test]
    fn enter_dedents_end_then_runs() {
        let buf = "local function f()\n  return 1\n  end";
        match enter(buf, buf.len()) {
            ReedlineEvent::Multiple(events) => assert_eq!(events.last(), Some(&ReedlineEvent::Enter)),
            e => panic!("{:?}", e),
        }
    }
}
