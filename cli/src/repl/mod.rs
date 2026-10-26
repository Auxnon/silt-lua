//! `silt` with no arguments: an interactive REPL in the style of Luau's. Each
//! input is tried as an expression first (`return <input>`) and its value printed,
//! otherwise run as a statement. Globals persist between inputs; locals, as in
//! Luau, last only for the input that declares them. Lives in the `silt` binary (`silt-cli`), not the library.

mod editor;
mod indent;
mod theme;

use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultHinter, EditCommand, Emacs,
    FileBackedHistory, KeyCode, KeyModifiers, MenuBuilder, Reedline, ReedlineEvent, ReedlineMenu,
    Signal,
};
use silt_lua::{error::ErrorOut, Compiler, ExVal, Lua};

use indent::SiltEditMode;
use editor::{Shared, SiltCompleter, SiltHighlighter, SiltPrompt, SiltValidator};
use theme::{Palette, Theme, NAMES};

const MAX_DEPTH: usize = 3;
const MAX_ENTRIES: usize = 40;

/// Start the REPL; `theme` is a palette name (`--theme`), else `$SILT_THEME`,
/// else `charm`. Piped (non-terminal) stdin is run as one script instead.
pub fn run(theme: Option<String>) {
    let plain = std::env::var_os("NO_COLOR").is_some() || !std::io::stdout().is_terminal();
    let name = theme
        .or_else(|| std::env::var("SILT_THEME").ok())
        .unwrap_or_else(|| "charm".into());
    let palette = Palette::named(&name).unwrap_or_else(|| {
        eprintln!("unknown theme '{}' (available: {})", name, NAMES.join(", "));
        Palette::named("charm").unwrap()
    });
    let mut repl = Repl::new(Theme::new(palette, plain));

    if !std::io::stdin().is_terminal() {
        let mut src = String::new();
        if std::io::stdin().read_to_string(&mut src).is_ok() {
            repl.eval(&src);
        }
        return;
    }
    repl.banner();
    repl.interactive();
}

struct Repl {
    lua: Lua,
    compiler: Compiler,
    shared: Shared,
}

impl Repl {
    fn new(theme: Theme) -> Repl {
        Repl {
            lua: Lua::new_with_standard(),
            compiler: Compiler::new_with_flags(true, true, false),
            shared: Shared::new(theme),
        }
    }

    fn theme(&self) -> Theme {
        self.shared.theme()
    }

    fn banner(&self) {
        let t = self.theme();
        let title = t.paint(t.badge(), format!(" silt {} ", env!("CARGO_PKG_VERSION")));
        let tag = t.paint(t.muted(), "Lua superset · gradual types");
        let hint = format!(
            "{} help  {} theme  {} exit",
            t.paint(t.primary(), ":help"),
            t.paint(t.primary(), ":theme"),
            t.paint(t.primary(), "ctrl-d"),
        );
        let bar = t.paint(t.secondary(), "│");
        println!();
        println!(" {} {}  {}", bar, title, tag);
        println!(" {} {}", bar, t.paint(t.faint(), hint));
        println!();
    }

    fn editor(&self) -> Reedline {
        let t = self.theme();
        let mut keys = default_emacs_keybindings();
        // Enter and Tab are decided by SiltEditMode (auto-indent)
        keys.add_binding(
            KeyModifiers::NONE,
            KeyCode::Enter,
            ReedlineEvent::ExecuteHostCommand(indent::ENTER.into()),
        );
        keys.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::ExecuteHostCommand(indent::TAB.into()),
        );
        // Alt+Enter: a newline without submitting
        keys.add_binding(
            KeyModifiers::ALT,
            KeyCode::Enter,
            ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
        );
        let menu = ColumnarMenu::default()
            .with_name("completion_menu")
            .with_marker("")
            .with_text_style(t.fg())
            .with_selected_text_style(t.selected())
            .with_description_text_style(t.faint())
            .with_match_text_style(t.primary().underline())
            .with_selected_match_text_style(t.selected().underline());
        let mut ed = Reedline::create()
            .with_highlighter(Box::new(SiltHighlighter(self.shared.clone())))
            .with_validator(Box::new(SiltValidator))
            .with_completer(Box::new(SiltCompleter(self.shared.clone())))
            .with_hinter(Box::new(DefaultHinter::default().with_style(t.faint())))
            .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)))
            .with_edit_mode(Box::new(SiltEditMode {
                inner: Emacs::new(keys),
                shared: self.shared.clone(),
            }))
            // a paste arrives whole, not as keystrokes auto-indent would re-indent
            .use_bracketed_paste(true)
            .with_quick_completions(false)
            .with_partial_completions(true)
            .with_ansi_colors(!t.plain);
        if let Some(path) = history_path() {
            if let Ok(h) = FileBackedHistory::with_file(1000, path) {
                ed = ed.with_history(Box::new(h));
            }
        }
        ed
    }

    fn interactive(&mut self) {
        let mut ed = self.editor();
        let prompt = SiltPrompt(self.shared.clone());
        loop {
            match ed.read_line(&prompt) {
                Ok(Signal::Success(line)) => {
                    let src = line.trim_end();
                    if src.trim().is_empty() {
                        continue;
                    }
                    if let Some(cmd) = src.trim().strip_prefix(':') {
                        match self.command(cmd) {
                            Command::Quit => break,
                            Command::Restyle => ed = self.editor(),
                            Command::Clear => {
                                let _ = ed.clear_screen();
                            }
                            Command::Done => {}
                        }
                        continue;
                    }
                    self.eval(src);
                }
                Ok(Signal::CtrlC) => continue,
                Ok(Signal::CtrlD) => break,
                Ok(_) => continue,
                Err(e) => {
                    eprintln!("{}", e);
                    break;
                }
            }
        }
    }

    fn command(&mut self, cmd: &str) -> Command {
        let t = self.theme();
        let mut words = cmd.split_whitespace();
        match words.next().unwrap_or("") {
            "q" | "quit" | "exit" => return Command::Quit,
            "clear" => return Command::Clear,
            "reset" => {
                self.lua = Lua::new_with_standard();
                self.compiler = Compiler::new_with_flags(true, true, false);
                self.shared.session.lock().unwrap().clear();
                println!("{}", t.paint(t.muted(), "fresh VM; globals cleared"));
            }
            "theme" => match words.next() {
                None => {
                    for n in NAMES {
                        let mark = if *n == t.palette.name { "●" } else { " " };
                        let p = Palette::named(n).unwrap();
                        let swatch = Theme::new(p, t.plain);
                        println!(
                            "  {} {}  {}{}{}",
                            t.paint(t.primary(), mark),
                            swatch.paint(swatch.primary(), format!("{:<7}", n)),
                            swatch.paint(swatch.primary(), "■"),
                            swatch.paint(swatch.secondary(), "■"),
                            swatch.paint(swatch.success(), "■"),
                        );
                    }
                }
                Some(n) => match Palette::named(n) {
                    Some(p) => {
                        *self.shared.theme.lock().unwrap() = Theme::new(p, t.plain);
                        return Command::Restyle;
                    }
                    None => self.fail(&format!(
                        "unknown theme '{}' (available: {})",
                        n,
                        NAMES.join(", ")
                    )),
                },
            },
            "help" | "h" | "?" => {
                let row = |k: &str, d: &str| {
                    println!("  {:<18} {}", t.paint(t.primary(), k), t.paint(t.muted(), d));
                };
                row(":help", "this list");
                row(":theme [name]", "list themes, or switch (charm dark light ocean sunset)");
                row(":reset", "start over with a fresh VM");
                row(":clear", "clear the screen");
                row(":quit", "exit (or ctrl-d)");
                row("tab", "complete globals, fields and keywords");
                row("alt-enter", "newline without running");
                println!(
                    "  {}",
                    t.paint(
                        t.faint(),
                        "expressions print their value; open blocks continue on `>>` \
                         (an empty line runs them anyway)"
                    )
                );
            }
            other => self.fail(&format!("unknown command ':{}' (try :help)", other)),
        }
        Command::Done
    }

    /// Run one input: as an expression when it is one, else as a statement.
    fn eval(&mut self, src: &str) {
        let as_expr = format!("return {}", src);
        let is_expr = silt_lua::lsp::diagnostics(&as_expr).is_empty();
        let code = if is_expr { as_expr.as_str() } else { src };
        match self.lua.run(None, code, &mut self.compiler) {
            Ok(v) => {
                let session = &mut *self.shared.session.lock().unwrap();
                session.push_str(src);
                session.push('\n');
                // a statement prints nothing, as in Luau: a chunk's result is just
                // whatever its last opcode left on the stack (e.g. after `function
                // f() end`). A call that returned nothing prints nothing either.
                let call = src.trim_end().ends_with(')');
                if is_expr && !(matches!(v, ExVal::Nil) && call) {
                    println!("{}", self.value(&v, 0));
                }
            }
            Err(e) => self.report(&e, src, if is_expr { 7 } else { 0 }),
        }
    }

    fn fail(&self, msg: &str) {
        let t = self.theme();
        println!("{} {}", t.paint(t.error().bold(), "✗"), t.paint(t.error(), msg));
    }

    /// Print each error with the offending line and a caret span. `shift` is how
    /// many columns `return ` added to the first line.
    fn report(&self, e: &ErrorOut, src: &str, shift: usize) {
        let t = self.theme();
        let lines: Vec<&str> = src.lines().collect();
        for err in &e.errors {
            self.fail(&err.code.to_string());
            let (line, col) = err.location;
            let Some(text) = line.checked_sub(1).and_then(|i| lines.get(i)) else {
                continue;
            };
            let fix = |l: usize, c: usize| if l == 1 { c.saturating_sub(shift) } else { c };
            let col = fix(line, col).max(1);
            let width = match err.end {
                Some((el, ec)) if el == line => (fix(el, ec) + 1).saturating_sub(col).max(1),
                Some(_) => text.chars().count().saturating_sub(col - 1).max(1),
                None => 1,
            };
            let gutter = format!("{:>3} │ ", line);
            println!("{}{}", t.paint(t.faint(), &gutter), highlight(&t, text));
            println!(
                "{}{}{}",
                " ".repeat(gutter.chars().count() - 2),
                t.paint(t.faint(), "│ "),
                t.paint(t.error(), format!("{}{}", " ".repeat(col - 1), "^".repeat(width)))
            );
        }
    }

    /// A value as Luau's REPL shows it: strings quoted, tables expanded.
    fn value(&self, v: &ExVal, depth: usize) -> String {
        let t = self.theme();
        match v {
            ExVal::Nil => t.paint(t.faint(), "nil"),
            ExVal::Bool(b) => t.paint(t.primary(), b.to_string()),
            ExVal::Integer(_) | ExVal::Number(_) | ExVal::Infinity(_) => {
                t.paint(t.number(), v.to_string())
            }
            ExVal::String(s) => t.paint(t.success(), format!("{:?}", s)),
            ExVal::Table(tab) => {
                if tab.is_empty() {
                    return t.paint(t.muted(), "{}");
                }
                if depth >= MAX_DEPTH {
                    return t.paint(t.muted(), "{…}");
                }
                // the sequence 1..n in order, then the other keys sorted
                let mut n = 0;
                while tab.getn(n + 1).is_some() {
                    n += 1;
                }
                let mut parts: Vec<String> =
                    (1..=n).map(|i| self.value(tab.getn(i).unwrap(), depth + 1)).collect();
                let mut rest: Vec<(String, String)> = tab
                    .iter()
                    .filter(|(k, _)| !matches!(k, ExVal::Integer(i) if *i >= 1 && *i as usize <= n))
                    .map(|(k, val)| {
                        let key = match k {
                            ExVal::String(s) if is_ident(s) => s.clone(),
                            k => format!("[{}]", self.value(k, MAX_DEPTH)),
                        };
                        (key, self.value(val, depth + 1))
                    })
                    .collect();
                rest.sort_by(|a, b| a.0.cmp(&b.0));
                parts.extend(rest.into_iter().map(|(k, v)| {
                    format!("{} {} {}", t.paint(t.fg(), k), t.paint(t.muted(), "="), v)
                }));
                let more = parts.len().saturating_sub(MAX_ENTRIES);
                parts.truncate(MAX_ENTRIES);
                if more > 0 {
                    parts.push(t.paint(t.faint(), format!("… {} more", more)));
                }
                let sep = t.paint(t.muted(), ", ");
                format!(
                    "{}{}{}",
                    t.paint(t.muted(), "{"),
                    parts.join(&sep),
                    t.paint(t.muted(), "}")
                )
            }
            other => t.paint(t.secondary(), other.to_string()),
        }
    }
}

enum Command {
    Done,
    Quit,
    Restyle,
    Clear,
}

fn is_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_alphabetic() || f == '_')
        && c.all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// One line of source with the REPL's syntax colours (for error snippets).
fn highlight(t: &Theme, text: &str) -> String {
    use reedline::Highlighter;
    SiltHighlighter(Shared::new(t.clone()))
        .highlight(text, 0)
        .buffer
        .into_iter()
        .map(|(style, s)| t.paint(style, s))
        .collect()
}

/// `$XDG_STATE_HOME/silt/history`, else `~/.local/state/silt/history`.
fn history_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("silt").join("history"))
}
