//! Language-server support for silt, in two layers:
//!
//! * **Analysis core** (feature `lsp`) — pure Rust, returns native types:
//!   [`diagnostics`] (compile errors) and [`format`] (Lua-style re-indentation).
//!   No serde, no JSON; an in-process Rust tool calls these directly.
//! * **Transport** (feature `lsp-server`) — a JSON-RPC-over-stdio server,
//!   [`run_server`], that wraps the core for editors like Neovim.
//!
//! Positions are LSP-style: 0-indexed line and 0-indexed character. silt's lexer
//! reports 1-indexed line/column, so we subtract one. Columns are Unicode-char
//! counts (accurate for ASCII/BMP text; astral-plane characters may drift from the
//! UTF-16 units LSP technically wants — a documented limitation).

use crate::token::Token;
use crate::{Compiler, Lua};

// ────────────────────────────── analysis core ──────────────────────────────

/// 0-indexed line + character (UTF-16-ish; see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A half-open span between two [`Position`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Diagnostic severity, mirroring the LSP severity ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A single compile diagnostic with its source span and message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub message: String,
}

/// Compile `source` (without executing it) and return every compile error as a
/// [`Diagnostic`]. A clean compile yields an empty vec (callers should publish that
/// to clear stale diagnostics). This is the in-process entry point — no JSON.
pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut lua = Lua::new();
    // Match the CLI's language-flag configuration (implicit return, arrow fns).
    let mut compiler = Compiler::new_with_flags(true, true, false);
    let result = lua.compile(Some("lsp"), source, &mut compiler);
    // `compile` (unlike `run`) doesn't drive GC, so reclaim the arena ourselves —
    // important under rapid didChange storms.
    lua.collect();

    match result {
        Ok(_) => Vec::new(),
        Err(out) => out
            .errors
            .iter()
            .map(|e| {
                // silt locations are 1-indexed (line, col); LSP wants 0-indexed.
                let (line, col) = e.location;
                let l = line.saturating_sub(1) as u32;
                let c = col.saturating_sub(1) as u32;
                Diagnostic {
                    // We only have a start point, so underline one character.
                    range: Range {
                        start: Position { line: l, character: c },
                        end: Position { line: l, character: c + 1 },
                    },
                    severity: Severity::Error,
                    message: format!("{}", e.code),
                }
            })
            .collect(),
    }
}

/// Re-indent `source` Lua-style using a tab per nesting level. See [`format_with`].
pub fn format(source: &str) -> String {
    format_with(source, "\t")
}

#[derive(Default, Clone)]
struct LineInfo {
    has_token: bool,
    /// First token of the line closes a block (dedents this line).
    first_is_closer: bool,
    /// Block-opening tokens on the line (`then`/`do`/`function`/`repeat`/`{`).
    opens: i32,
    /// Block-closing tokens on the line (`end`/`until`/`}`).
    closes: i32,
    /// Line contains `elseif`, whose trailing `then` must not add a net level.
    has_elseif: bool,
}

/// Re-indent `source` using `indent_unit` (e.g. `"\t"` or `"  "`) per nesting level.
///
/// This is a token-driven *re-indenter*, not a full pretty-printer: it normalizes
/// leading indentation for block constructs (functions, if/elseif/else/end,
/// for/while/do, repeat/until, `{ }` tables) and preserves each line's own content
/// and inline spacing. Multi-line strings/comments are emitted verbatim. If the
/// source doesn't lex cleanly it is returned unchanged (formatting a broken buffer
/// is a no-op, as editors expect). Operator-spacing normalization and alignment are
/// intentionally out of scope.
pub fn format_with(source: &str, indent_unit: &str) -> String {
    let src_lines: Vec<&str> = source.split('\n').collect();
    let line_count = src_lines.len();
    let mut info = vec![LineInfo::default(); line_count];
    // Lines that fall *inside* a multi-line token (long string / block comment) and
    // must be passed through untouched.
    let mut verbatim = vec![false; line_count];

    for token_result in crate::lexer::Lexer::new(source) {
        let (token, triple) = match token_result {
            Ok(t) => t,
            // Malformed source: bail and leave the buffer exactly as-is.
            Err(_) => return source.to_string(),
        };
        if matches!(token, Token::EOF) {
            continue;
        }
        let idx = triple.line.saturating_sub(1); // 0-indexed start line
        if idx >= line_count {
            continue;
        }

        // A token whose text spans newlines (multi-line string/comment) makes its
        // continuation lines verbatim. Use a checked slice so a non-UTF8-boundary
        // (multi-byte) offset can't panic.
        let end = triple.index + triple.length;
        if let Some(slice) = source.get(triple.index..end) {
            let nl = slice.matches('\n').count();
            for k in 1..=nl {
                if idx + k < line_count {
                    verbatim[idx + k] = true;
                }
            }
        }

        let li = &mut info[idx];
        if !li.has_token {
            li.has_token = true;
            li.first_is_closer = matches!(
                token,
                Token::End | Token::Until | Token::Else | Token::ElseIf | Token::CloseBrace
            );
        }
        match token {
            Token::Then | Token::Do | Token::Function | Token::Repeat | Token::OpenBrace => {
                li.opens += 1
            }
            Token::End | Token::Until | Token::CloseBrace => li.closes += 1,
            Token::ElseIf => li.has_elseif = true,
            _ => {}
        }
    }

    let mut out = String::new();
    let mut depth: i32 = 0;
    for (i, raw) in src_lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if verbatim[i] {
            out.push_str(raw); // interior of a multi-line token — never reindent
            continue;
        }
        let trimmed = raw.trim_end_matches('\r').trim_start();
        if trimmed.is_empty() {
            continue; // preserve blank lines without indentation
        }
        let li = &info[i];
        let dedent = if li.first_is_closer { 1 } else { 0 };
        let this_indent = (depth - dedent).max(0);
        for _ in 0..this_indent {
            out.push_str(indent_unit);
        }
        out.push_str(trimmed);
        let net = li.opens - li.closes - if li.has_elseif { 1 } else { 0 };
        depth = (depth + net).max(0);
    }
    out
}

// ──────────────────────────── JSON-RPC transport ────────────────────────────

#[cfg(feature = "lsp-server")]
mod server {
    use super::{diagnostics, format_with, Diagnostic, Severity};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::io::{BufRead, Read, Write};

    fn severity_code(s: Severity) -> i64 {
        match s {
            Severity::Error => 1,
            Severity::Warning => 2,
            Severity::Information => 3,
            Severity::Hint => 4,
        }
    }

    /// Read one `Content-Length`-framed JSON-RPC message. `None` on clean EOF.
    fn read_message<R: BufRead>(reader: &mut R) -> Option<Value> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).ok()?;
            if n == 0 {
                return None; // EOF — client gone
            }
            let trimmed = line.trim_end(); // drop \r\n
            if trimmed.is_empty() {
                break; // blank line ends the header block
            }
            let lower = trimmed.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                content_length = rest.trim().parse().ok();
            }
        }
        let len = content_length?;
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).ok()?;
        serde_json::from_slice(&buf).ok()
    }

    fn write_message<W: Write>(writer: &mut W, value: &Value) {
        let body = serde_json::to_vec(value).unwrap_or_default();
        // Content-Length is the byte length of the JSON body.
        let _ = write!(writer, "Content-Length: {}\r\n\r\n", body.len());
        let _ = writer.write_all(&body);
        let _ = writer.flush(); // a missing flush is the classic "server hangs"
    }

    fn send_response<W: Write>(writer: &mut W, id: Value, result: Value) {
        write_message(
            writer,
            &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        );
    }

    fn send_error<W: Write>(writer: &mut W, id: Value, code: i64, message: &str) {
        write_message(
            writer,
            &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
        );
    }

    fn diagnostic_json(d: &Diagnostic) -> Value {
        json!({
            "range": {
                "start": { "line": d.range.start.line, "character": d.range.start.character },
                "end":   { "line": d.range.end.line,   "character": d.range.end.character },
            },
            "severity": severity_code(d.severity),
            "source": "silt",
            "message": d.message,
        })
    }

    fn publish<W: Write>(writer: &mut W, uri: &str, diags: &[Diagnostic]) {
        let arr: Vec<Value> = diags.iter().map(diagnostic_json).collect();
        write_message(
            writer,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/publishDiagnostics",
                "params": { "uri": uri, "diagnostics": arr },
            }),
        );
    }

    /// A single whole-document replacement edit (LSP formatting returns edits).
    fn full_document_edit(old: &str, formatted: String) -> Value {
        let line_count = old.split('\n').count() as u32;
        json!({
            "range": {
                "start": { "line": 0, "character": 0 },
                // One line past the last covers the whole buffer; clients clamp.
                "end":   { "line": line_count, "character": 0 },
            },
            "newText": formatted,
        })
    }

    /// Run the stdio JSON-RPC language server until the client disconnects.
    /// All logging goes to stderr; stdout carries only framed JSON-RPC.
    pub fn run() {
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        let stdout = std::io::stdout();
        let mut writer = stdout.lock();

        let mut docs: HashMap<String, String> = HashMap::new();
        let mut shutdown_requested = false;

        while let Some(msg) = read_message(&mut reader) {
            let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
            let id = msg.get("id").cloned();

            match method {
                "initialize" => {
                    let result = json!({
                        "capabilities": {
                            "textDocumentSync": 1, // Full
                            "documentFormattingProvider": true,
                        },
                        "serverInfo": { "name": "silt", "version": env!("CARGO_PKG_VERSION") },
                    });
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "initialized" => {}
                "shutdown" => {
                    shutdown_requested = true;
                    send_response(&mut writer, id.unwrap_or(Value::Null), Value::Null);
                }
                "exit" => break,
                "textDocument/didOpen" => {
                    let td = &msg["params"]["textDocument"];
                    if let (Some(uri), Some(text)) = (td["uri"].as_str(), td["text"].as_str()) {
                        let uri = uri.to_string();
                        let diags = diagnostics(text);
                        docs.insert(uri.clone(), text.to_string());
                        publish(&mut writer, &uri, &diags);
                    }
                }
                "textDocument/didChange" => {
                    let uri = msg["params"]["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    // Full sync: the last content change holds the whole document.
                    if let Some(text) = msg["params"]["contentChanges"]
                        .as_array()
                        .and_then(|c| c.last())
                        .and_then(|c| c["text"].as_str())
                    {
                        let diags = diagnostics(text);
                        docs.insert(uri.clone(), text.to_string());
                        publish(&mut writer, &uri, &diags);
                    }
                }
                "textDocument/didClose" => {
                    let uri = msg["params"]["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    docs.remove(&uri);
                    publish(&mut writer, &uri, &[]); // clear stale diagnostics
                }
                "textDocument/formatting" => {
                    let uri = msg["params"]["textDocument"]["uri"].as_str().unwrap_or("");
                    let opts = &msg["params"]["options"];
                    let insert_spaces = opts["insertSpaces"].as_bool().unwrap_or(true);
                    let tab_size = opts["tabSize"].as_u64().unwrap_or(4) as usize;
                    let unit = if insert_spaces {
                        " ".repeat(tab_size)
                    } else {
                        "\t".to_string()
                    };
                    let edits = match docs.get(uri) {
                        Some(text) => {
                            let formatted = format_with(text, &unit);
                            json!([full_document_edit(text, formatted)])
                        }
                        None => json!([]),
                    };
                    send_response(&mut writer, id.unwrap_or(Value::Null), edits);
                }
                // Unknown request: must answer; unknown notification: ignore.
                _ => {
                    if let Some(id) = id {
                        send_error(&mut writer, id, -32601, "method not found");
                    }
                }
            }
        }

        if !shutdown_requested {
            eprintln!("silt-lsp: client disconnected without shutdown");
        }
    }
}

#[cfg(feature = "lsp-server")]
pub use server::run as run_server;
