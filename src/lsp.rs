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

mod analysis;
pub use analysis::{analyze, Analysis, Reference, Symbol, SymbolKind};

// ────────────────────────────── analysis core ──────────────────────────────

/// 0-indexed line + character (UTF-16-ish; see module docs). Ordered by line, then character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
                // The compiler records the inclusive end of the failing span when it
                // has one; otherwise underline a single character.
                let end = match e.end {
                    Some((el, ec)) => Position {
                        line: el.saturating_sub(1) as u32,
                        character: ec as u32,
                    },
                    None => Position {
                        line: l,
                        character: c + 1,
                    },
                };
                Diagnostic {
                    range: Range {
                        start: Position {
                            line: l,
                            character: c,
                        },
                        end,
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

// ───────────────────────────── editor features ─────────────────────────────

/// Hover text for the symbol at `pos` (markdown): its kind, signature and
/// declared type, plus the doc comment above its definition.
pub fn hover(source: &str, pos: Position) -> Option<String> {
    let an = analyze(source);
    let sym = an.symbol_at(pos).map(|i| &an.symbols[i]);
    match sym {
        Some(sym) => Some(describe(sym)),
        None => {
            // an undeclared name: maybe a standard-library global
            let i = an.token_at(pos)?;
            let name = match &an.tokens[i].token {
                Token::Identifier(n) => n.clone(),
                _ => return None,
            };
            if stdlib_names().iter().any(|n| n == &name) {
                Some(format!("```lua\n(global) {}\n```\nstandard library", name))
            } else {
                None
            }
        }
    }
}

fn describe(sym: &Symbol) -> String {
    let ret = sym
        .returns
        .as_ref()
        .map(|r| format!(": {}", r))
        .unwrap_or_default();
    let head = match sym.kind {
        SymbolKind::Function => format!(
            "function {}({}){}",
            sym.name,
            sym.params.clone().unwrap_or_default(),
            ret
        ),
        SymbolKind::LocalFunction => format!(
            "local function {}({}){}",
            sym.name,
            sym.params.clone().unwrap_or_default(),
            ret
        ),
        SymbolKind::Method => format!(
            "function {}:{}({}){}",
            sym.receiver.clone().unwrap_or_default(),
            sym.name,
            sym.params.clone().unwrap_or_default(),
            ret
        ),
        SymbolKind::Field => format!(
            "function {}.{}({}){}",
            sym.receiver.clone().unwrap_or_default(),
            sym.name,
            sym.params.clone().unwrap_or_default(),
            ret
        ),
        SymbolKind::Local => format!("local {}{}", sym.name, annot(sym)),
        SymbolKind::Parameter => format!("(parameter) {}{}", sym.name, annot(sym)),
        SymbolKind::LoopVariable => format!("(loop variable) {}", sym.name),
        SymbolKind::Global => format!("(global) {}", sym.name),
        SymbolKind::TypeAlias => format!(
            "type {} = {}",
            sym.name,
            sym.alias_of.clone().unwrap_or_default()
        ),
    };
    let mut out = format!("```lua\n{}\n```", head);
    if let Some(doc) = &sym.doc {
        out.push_str("\n\n");
        out.push_str(doc);
    }
    out
}

fn annot(sym: &Symbol) -> String {
    sym.ty
        .as_ref()
        .map(|t| format!(": {}", t))
        .unwrap_or_default()
}

/// Where the symbol at `pos` is defined.
pub fn definition(source: &str, pos: Position) -> Option<Range> {
    let an = analyze(source);
    an.symbol_at(pos).map(|i| an.symbols[i].range)
}

/// Every occurrence (definition + uses) of the symbol at `pos`.
pub fn references(source: &str, pos: Position) -> Vec<Range> {
    let an = analyze(source);
    match an.symbol_at(pos) {
        Some(i) => an.occurrences(i),
        None => vec![],
    }
}

/// Ranges to rewrite when renaming the symbol at `pos`. Empty when the
/// position is not on a renameable symbol.
pub fn rename(source: &str, pos: Position) -> Vec<Range> {
    references(source, pos)
}

/// A completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Variable,
    Function,
    Field,
    Keyword,
    Type,
    Module,
}

const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while", "type",
];

const TYPE_NAMES: &[&str] = &[
    "any", "nil", "boolean", "number", "string", "table", "function",
];

/// Names registered by `Lua::new_with_standard`, plus `lib.member` for every
/// table-valued global (`string.upper`, `table.insert`, `math.floor`, …).
pub fn stdlib_names() -> Vec<String> {
    let mut lua = Lua::new_with_standard();
    let mut names: Vec<String> = vec![];
    lua.enter(|vm, _mc| {
        let globals = vm.globals.borrow();
        for (k, v) in globals.iter() {
            if let crate::value::Value::String(name) = k {
                names.push(name.clone());
                if let crate::value::Value::Table(t) = v {
                    for (fk, _) in t.borrow().iter() {
                        if let crate::value::Value::String(f) = fk {
                            names.push(format!("{}.{}", name, f));
                        }
                    }
                }
            }
        }
    });
    names.sort();
    names
}

/// Completion candidates at `pos`: after `recv.` / `recv:` the members seen on
/// that receiver (and the library's if it is a stdlib table); otherwise the
/// symbols in scope, globals, stdlib names, type names (after `:`) and keywords.
pub fn completion(source: &str, pos: Position) -> Vec<Completion> {
    let an = analyze(source);
    let offset = Analysis::offset_of(source, pos);
    let before = &source[..offset.min(source.len())];
    // the word being typed, and what precedes it
    let word_start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map(|i| i + 1)
        .unwrap_or(0);
    let prefix = &before[word_start..];
    let lead = before[..word_start].trim_end();
    let mut out: Vec<Completion> = vec![];
    let push =
        |out: &mut Vec<Completion>, label: &str, kind: CompletionKind, detail: Option<String>| {
            if label.starts_with(prefix) && !out.iter().any(|c| c.label == label) {
                out.push(Completion {
                    label: label.to_string(),
                    kind,
                    detail,
                });
            }
        };

    // `local x: `, `(a: `, `, b: `, `): ` introduce a type; `t:` is a method access.
    let annotation_colon = lead.ends_with(':') && {
        let r = lead[..lead.len() - 1].trim_end();
        if r.ends_with(')') {
            true
        } else {
            let recv_start = r
                .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
                .map(|i| i + 1)
                .unwrap_or(0);
            let before_recv = r[..recv_start].trim_end();
            before_recv.ends_with("local")
                || before_recv.ends_with(',')
                || before_recv.ends_with('(')
        }
    };
    if (lead.ends_with('.') || lead.ends_with(':')) && !annotation_colon {
        // member completion on `recv`
        let recv_end = lead.len() - 1;
        let recv_start = lead[..recv_end]
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
            .map(|i| i + 1)
            .unwrap_or(0);
        let recv = &lead[recv_start..recv_end];
        if let Some(fields) = an.fields.get(recv) {
            for f in fields {
                push(
                    &mut out,
                    f,
                    CompletionKind::Field,
                    Some(format!("{}.{}", recv, f)),
                );
            }
        }
        let lib_prefix = format!("{}.", recv);
        for n in stdlib_names() {
            if let Some(member) = n.strip_prefix(&lib_prefix) {
                push(&mut out, member, CompletionKind::Function, Some(n.clone()));
            }
        }
        // a receiver typed with a table shape (directly or via an alias): its fields
        if let Some(sym) = an.symbols.iter().rev().find(|s| s.name == recv) {
            if let Some(ty) = &sym.ty {
                let shape = resolve_alias_text(&an, ty);
                for field in shape_fields(&shape) {
                    push(&mut out, &field, CompletionKind::Field, Some(shape.clone()));
                }
            }
        }
        return out;
    }

    // `local x: ` / `(a: ` — a type position
    let type_position = annotation_colon || lead.ends_with("->") || lead.ends_with('|');
    if type_position {
        for t in TYPE_NAMES {
            push(&mut out, t, CompletionKind::Type, None);
        }
        for s in an
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::TypeAlias)
        {
            push(&mut out, &s.name, CompletionKind::Type, s.alias_of.clone());
        }
        return out;
    }

    for sym in an.visible_at(offset) {
        let kind = match sym.kind {
            SymbolKind::Function
            | SymbolKind::LocalFunction
            | SymbolKind::Method
            | SymbolKind::Field => CompletionKind::Function,
            SymbolKind::TypeAlias => CompletionKind::Type,
            _ => CompletionKind::Variable,
        };
        if matches!(sym.kind, SymbolKind::Method | SymbolKind::Field) {
            continue; // reached through their receiver
        }
        push(&mut out, &sym.name, kind, Some(describe_short(sym)));
    }
    for n in stdlib_names() {
        if !n.contains('.') {
            let kind = if an.fields.contains_key(&n)
                || stdlib_names()
                    .iter()
                    .any(|m| m.starts_with(&format!("{}.", n)))
            {
                CompletionKind::Module
            } else {
                CompletionKind::Function
            };
            push(&mut out, &n, kind, Some("standard library".into()));
        }
    }
    for k in KEYWORDS {
        push(&mut out, k, CompletionKind::Keyword, None);
    }
    out
}

fn describe_short(sym: &Symbol) -> String {
    match sym.kind {
        SymbolKind::Function | SymbolKind::LocalFunction => {
            format!("function({})", sym.params.clone().unwrap_or_default())
        }
        SymbolKind::TypeAlias => format!("type = {}", sym.alias_of.clone().unwrap_or_default()),
        _ => sym
            .ty
            .clone()
            .unwrap_or_else(|| format!("{:?}", sym.kind).to_lowercase()),
    }
}

/// Follow `type` aliases by name until the text is not a bare alias name.
fn resolve_alias_text(an: &Analysis, ty: &str) -> String {
    let mut cur = ty.trim().to_string();
    for _ in 0..6 {
        let next = an
            .symbols
            .iter()
            .find(|s| s.kind == SymbolKind::TypeAlias && s.name == cur)
            .and_then(|s| s.alias_of.clone());
        match next {
            Some(n) => cur = n.trim().to_string(),
            None => break,
        }
    }
    cur
}

/// Field names of a `{a: T, b: U}` shape annotation.
fn shape_fields(ty: &str) -> Vec<String> {
    let t = ty.trim();
    if !t.starts_with('{') {
        return vec![];
    }
    let inner = t.trim_start_matches('{').trim_end_matches('}');
    let mut out = vec![];
    let mut depth = 0i32;
    let mut field = String::new();
    let mut at_name = true;
    for c in inner.chars() {
        match c {
            '{' | '(' | '[' => {
                depth += 1;
                at_name = false;
            }
            '}' | ')' | ']' => depth -= 1,
            ',' | ';' if depth == 0 => {
                if !field.trim().is_empty() {
                    out.push(field.trim().to_string());
                }
                field.clear();
                at_name = true;
            }
            ':' if depth == 0 => at_name = false,
            c if at_name && (c.is_alphanumeric() || c == '_') => field.push(c),
            _ => {}
        }
    }
    if !field.trim().is_empty() {
        out.push(field.trim().to_string());
    }
    out
}

/// An outline entry (LSP `DocumentSymbol`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub detail: Option<String>,
}

/// Functions, methods, globals, type aliases and top-level locals, in source order.
pub fn document_symbols(source: &str) -> Vec<DocumentSymbol> {
    let an = analyze(source);
    an.symbols
        .iter()
        .filter(|s| match s.kind {
            SymbolKind::Parameter | SymbolKind::LoopVariable => false,
            SymbolKind::Local => s.scope == 0,
            _ => true,
        })
        .filter(|s| !(s.kind == SymbolKind::Parameter))
        .map(|s| DocumentSymbol {
            name: match s.kind {
                SymbolKind::Method | SymbolKind::Field => {
                    format!(
                        "{}{}{}",
                        s.receiver.clone().unwrap_or_default(),
                        if s.kind == SymbolKind::Method {
                            ":"
                        } else {
                            "."
                        },
                        s.name
                    )
                }
                _ => s.name.clone(),
            },
            kind: s.kind,
            range: s.range,
            detail: Some(describe_short(s)),
        })
        .collect()
}

/// Signature help inside a call: the callee's parameter list and which
/// parameter the cursor is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub label: String,
    pub params: Vec<String>,
    pub active_parameter: u32,
}

pub fn signature_help(source: &str, pos: Position) -> Option<SignatureHelp> {
    let an = analyze(source);
    let offset = Analysis::offset_of(source, pos);
    // walk back over tokens to the unmatched `(`, counting commas at depth 0
    let mut depth = 0i32;
    let mut commas = 0u32;
    let mut open: Option<usize> = None;
    for (i, t) in an.tokens.iter().enumerate().rev() {
        if t.offset >= offset {
            continue;
        }
        match t.token {
            Token::CloseParen | Token::CloseBrace | Token::CloseBracket => depth += 1,
            Token::OpenParen => {
                if depth == 0 {
                    open = Some(i);
                    break;
                }
                depth -= 1;
            }
            Token::OpenBrace | Token::OpenBracket => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            Token::Comma if depth == 0 => commas += 1,
            _ => {}
        }
    }
    let open = open?;
    if open == 0 {
        return None;
    }
    let callee = &an.tokens[open - 1];
    let name = match &callee.token {
        Token::Identifier(n) => n.clone(),
        _ => return None,
    };
    let sym = an.symbol_at(callee.range.start).map(|i| &an.symbols[i]);
    let sym = match sym {
        Some(s) if s.params.is_some() => s,
        _ => return None,
    };
    let params_text = sym.params.clone().unwrap_or_default();
    let params: Vec<String> = split_params(&params_text);
    let is_method = open >= 2 && matches!(an.tokens[open - 2].token, Token::Colon);
    let shown: Vec<String> = if is_method && params.first().map(|p| p == "self").unwrap_or(false) {
        params[1..].to_vec()
    } else {
        params.clone()
    };
    Some(SignatureHelp {
        label: format!("{}({})", name, shown.join(", ")),
        params: shown,
        active_parameter: commas,
    })
}

fn split_params(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in text.chars() {
        match c {
            '{' | '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            '}' | ')' | ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Semantic token types, in legend order.
pub const SEMANTIC_TOKEN_TYPES: &[&str] = &[
    "keyword",
    "string",
    "number",
    "comment",
    "operator",
    "function",
    "method",
    "parameter",
    "variable",
    "property",
    "type",
];

/// One semantic token: `(line, start, length, type index)` — 0-indexed, in
/// source order. Encode to the LSP delta format in the transport.
pub fn semantic_tokens(source: &str) -> Vec<(u32, u32, u32, u32)> {
    let an = analyze(source);
    let idx = |name: &str| {
        SEMANTIC_TOKEN_TYPES
            .iter()
            .position(|t| *t == name)
            .unwrap_or(8) as u32
    };
    let mut out = vec![];
    for (i, t) in an.tokens.iter().enumerate() {
        let kind = match &t.token {
            Token::StringLiteral(_) => Some("string"),
            Token::Number(_) | Token::Integer(_) => Some("number"),
            Token::Comment => Some("comment"),
            Token::Op(_) => Some("operator"),
            Token::Identifier(name) => {
                let pos = t.range.start;
                let sym = an.symbols.iter().find(|s| s.range == t.range).or_else(|| {
                    an.refs
                        .iter()
                        .find(|r| r.range == t.range)
                        .and_then(|r| r.symbol)
                        .map(|i| &an.symbols[i])
                });
                Some(match sym.map(|s| s.kind) {
                    Some(SymbolKind::Function | SymbolKind::LocalFunction | SymbolKind::Field) => {
                        "function"
                    }
                    Some(SymbolKind::Method) => "method",
                    Some(SymbolKind::Parameter) => "parameter",
                    Some(SymbolKind::TypeAlias) => "type",
                    Some(_) => "variable",
                    None => {
                        let after_sep =
                            i > 0 && matches!(an.tokens[i - 1].token, Token::Dot | Token::Colon);
                        let called = matches!(
                            an.tokens.get(i + 1).map(|t| &t.token),
                            Some(Token::OpenParen | Token::StringLiteral(_) | Token::OpenBrace)
                        );
                        if name == "type"
                            && matches!(
                                an.tokens.get(i + 1).map(|t| &t.token),
                                Some(Token::Identifier(_))
                            )
                        {
                            "keyword"
                        } else if after_sep {
                            if called {
                                "method"
                            } else {
                                "property"
                            }
                        } else if called {
                            "function"
                        } else {
                            let _ = pos;
                            "variable"
                        }
                    }
                })
            }
            Token::Nil
            | Token::True
            | Token::False
            | Token::Local
            | Token::Function
            | Token::If
            | Token::Then
            | Token::Else
            | Token::ElseIf
            | Token::End
            | Token::For
            | Token::In
            | Token::While
            | Token::Do
            | Token::Repeat
            | Token::Until
            | Token::Return
            | Token::Break
            | Token::Goto
            | Token::Global
            | Token::Class
            | Token::Flag(_) => Some("keyword"),
            _ => None,
        };
        if let Some(k) = kind {
            if t.range.start.line == t.range.end.line {
                out.push((
                    t.range.start.line,
                    t.range.start.character,
                    t.len as u32,
                    idx(k),
                ));
            }
        }
    }
    out
}

// ──────────────────────────── JSON-RPC transport ────────────────────────────

#[cfg(feature = "lsp-server")]
mod server {
    use super::{
        completion, definition, diagnostics, document_symbols, format_with, hover, references,
        rename, semantic_tokens, signature_help, CompletionKind, Diagnostic, Position, Range,
        Severity, SymbolKind, SEMANTIC_TOKEN_TYPES,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::io::{BufRead, Write};

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

    fn doc_position(msg: &Value) -> (String, Position) {
        let uri = msg["params"]["textDocument"]["uri"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let p = &msg["params"]["position"];
        (
            uri,
            Position {
                line: p["line"].as_u64().unwrap_or(0) as u32,
                character: p["character"].as_u64().unwrap_or(0) as u32,
            },
        )
    }

    fn range_json(r: &Range) -> Value {
        json!({
            "start": { "line": r.start.line, "character": r.start.character },
            "end":   { "line": r.end.line,   "character": r.end.character },
        })
    }

    /// LSP `SymbolKind` codes.
    fn symbol_kind_code(k: SymbolKind) -> i64 {
        match k {
            SymbolKind::Function | SymbolKind::LocalFunction | SymbolKind::Field => 12,
            SymbolKind::Method => 6,
            SymbolKind::TypeAlias => 5, // Class
            _ => 13,                    // Variable
        }
    }

    /// LSP `CompletionItemKind` codes.
    fn completion_kind_code(k: CompletionKind) -> i64 {
        match k {
            CompletionKind::Variable => 6,
            CompletionKind::Function => 3,
            CompletionKind::Field => 5,
            CompletionKind::Keyword => 14,
            CompletionKind::Type => 7,
            CompletionKind::Module => 9,
        }
    }

    /// LSP semantic-token delta encoding: each token is 5 ints relative to the previous.
    fn encode_semantic_tokens(toks: &[(u32, u32, u32, u32)]) -> Vec<u32> {
        let mut out = Vec::with_capacity(toks.len() * 5);
        let (mut pl, mut pc) = (0u32, 0u32);
        for &(line, ch, len, ty) in toks {
            let dl = line - pl;
            let dc = if dl == 0 { ch - pc } else { ch };
            out.extend_from_slice(&[dl, dc, len, ty, 0]);
            pl = line;
            pc = ch;
        }
        out
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
                            "hoverProvider": true,
                            "definitionProvider": true,
                            "referencesProvider": true,
                            "renameProvider": true,
                            "documentSymbolProvider": true,
                            "completionProvider": { "triggerCharacters": [".", ":"] },
                            "signatureHelpProvider": { "triggerCharacters": ["(", ","] },
                            "semanticTokensProvider": {
                                "legend": { "tokenTypes": SEMANTIC_TOKEN_TYPES, "tokenModifiers": [] },
                                "full": true,
                            },
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
                "textDocument/hover" => {
                    let (uri, pos) = doc_position(&msg);
                    let result = docs
                        .get(&uri)
                        .and_then(|text| hover(text, pos))
                        .map(|md| json!({ "contents": { "kind": "markdown", "value": md } }))
                        .unwrap_or(Value::Null);
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "textDocument/definition" => {
                    let (uri, pos) = doc_position(&msg);
                    let result = docs
                        .get(&uri)
                        .and_then(|text| definition(text, pos))
                        .map(|r| json!({ "uri": uri, "range": range_json(&r) }))
                        .unwrap_or(Value::Null);
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "textDocument/references" => {
                    let (uri, pos) = doc_position(&msg);
                    let locs: Vec<Value> = docs
                        .get(&uri)
                        .map(|text| references(text, pos))
                        .unwrap_or_default()
                        .iter()
                        .map(|r| json!({ "uri": uri, "range": range_json(r) }))
                        .collect();
                    send_response(&mut writer, id.unwrap_or(Value::Null), json!(locs));
                }
                "textDocument/rename" => {
                    let (uri, pos) = doc_position(&msg);
                    let new_name = msg["params"]["newName"].as_str().unwrap_or("").to_string();
                    let edits: Vec<Value> = docs
                        .get(&uri)
                        .map(|text| rename(text, pos))
                        .unwrap_or_default()
                        .iter()
                        .map(|r| json!({ "range": range_json(r), "newText": new_name }))
                        .collect();
                    let result = if edits.is_empty() {
                        Value::Null
                    } else {
                        json!({ "changes": { uri: edits } })
                    };
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "textDocument/documentSymbol" => {
                    let uri = msg["params"]["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let syms: Vec<Value> = docs
                        .get(&uri)
                        .map(|text| document_symbols(text))
                        .unwrap_or_default()
                        .iter()
                        .map(|s| {
                            json!({
                                "name": s.name,
                                "kind": symbol_kind_code(s.kind),
                                "range": range_json(&s.range),
                                "selectionRange": range_json(&s.range),
                                "detail": s.detail,
                            })
                        })
                        .collect();
                    send_response(&mut writer, id.unwrap_or(Value::Null), json!(syms));
                }
                "textDocument/completion" => {
                    let (uri, pos) = doc_position(&msg);
                    let items: Vec<Value> = docs
                        .get(&uri)
                        .map(|text| completion(text, pos))
                        .unwrap_or_default()
                        .iter()
                        .map(|c| {
                            json!({
                                "label": c.label,
                                "kind": completion_kind_code(c.kind),
                                "detail": c.detail,
                            })
                        })
                        .collect();
                    send_response(
                        &mut writer,
                        id.unwrap_or(Value::Null),
                        json!({ "isIncomplete": false, "items": items }),
                    );
                }
                "textDocument/signatureHelp" => {
                    let (uri, pos) = doc_position(&msg);
                    let result = docs
                        .get(&uri)
                        .and_then(|text| signature_help(text, pos))
                        .map(|sh| {
                            json!({
                                "signatures": [{
                                    "label": sh.label,
                                    "parameters": sh.params.iter().map(|p| json!({ "label": p })).collect::<Vec<_>>(),
                                }],
                                "activeSignature": 0,
                                "activeParameter": sh.active_parameter,
                            })
                        })
                        .unwrap_or(Value::Null);
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "textDocument/semanticTokens/full" => {
                    let uri = msg["params"]["textDocument"]["uri"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let data = docs
                        .get(&uri)
                        .map(|text| encode_semantic_tokens(&semantic_tokens(text)))
                        .unwrap_or_default();
                    send_response(
                        &mut writer,
                        id.unwrap_or(Value::Null),
                        json!({ "data": data }),
                    );
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
