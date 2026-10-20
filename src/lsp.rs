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

use std::collections::BTreeMap;

use crate::token::{Operator, Token};
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
    diagnostics_with(source, |_| {})
}

/// [`diagnostics`] with a hook to configure the compiler first — typically the
/// host's type declarations (`compiler.declare("instr", "...")`, `typing` feature),
/// so a script is checked against the natives it will actually run with.
pub fn diagnostics_with(source: &str, configure: impl FnOnce(&mut Compiler)) -> Vec<Diagnostic> {
    let mut lua = Lua::new();
    // Match the CLI's language-flag configuration (implicit return, arrow fns).
    let mut compiler = Compiler::new_with_flags(true, true, false);
    configure(&mut compiler);
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
    hover_in(source, "", pos, &Library::default())
}

/// [`hover`] for document `uri`, falling back to `lib`'s definitions (and the
/// runtime's documented natives) for names the document doesn't define.
pub fn hover_in(source: &str, uri: &str, pos: Position, lib: &Library) -> Option<String> {
    let an = analyze(source);
    let i = match an.symbol_at(pos) {
        Some(s) => return Some(describe(&with_inferred_type(&an, s, uri, lib))),
        None => an.token_at(pos)?,
    };
    match lookup(&an, i, uri, lib)? {
        Found::Local(sym) => Some(describe(sym)),
        Found::Lib(file, sym) => {
            let name = file.rsplit('/').next().unwrap_or(file);
            Some(format!("{}\n\n*{}*", describe(sym), name))
        }
        Found::Builtin(b) => Some(b.describe()),
        Found::Stdlib(name) => Some(format!("```lua\n(global) {}\n```\nstandard library", name)),
    }
}

/// Symbol `s` with a best-effort type when it has no annotation (see [`infer_symbol`]).
fn with_inferred_type(an: &Analysis, s: usize, uri: &str, lib: &Library) -> Symbol {
    let mut sym = an.symbols[s].clone();
    if sym.ty.is_none() {
        sym.ty = infer_symbol(an, s, uri, lib, 0);
    }
    sym
}

/// Best-effort type of an unannotated local/global, from the first assignment to it
/// that says something (`local x` / `X = nil` then `X = vec3(…)` finds `vec3`).
fn infer_symbol(an: &Analysis, s: usize, uri: &str, lib: &Library, depth: u8) -> Option<String> {
    let sym = &an.symbols[s];
    if depth > 4 || !matches!(sym.kind, SymbolKind::Local | SymbolKind::Global) {
        return sym.ty.clone();
    }
    if sym.ty.is_some() {
        return sym.ty.clone();
    }
    for r in an.occurrences(s) {
        let Some(i) = an.tokens.iter().position(|t| t.range == r) else { continue };
        let assigned = matches!(
            an.tokens.get(i + 1).map(|t| &t.token),
            Some(Token::Assign | Token::Op(Operator::ColonEquals))
        );
        let member = i > 0 && matches!(an.tokens[i - 1].token, Token::Dot | Token::Colon);
        if assigned && !member {
            if let Some(ty) = expr_type(an, i + 2, uri, lib, depth + 1) {
                return Some(ty);
            }
        }
    }
    None
}

/// Type of the expression starting at token `i`: a literal, a call's return type,
/// a typed variable, or arithmetic / concatenation / comparison on those.
fn expr_type(an: &Analysis, i: usize, uri: &str, lib: &Library, depth: u8) -> Option<String> {
    let (ty, next) = primary_type(an, i, uri, lib, depth)?;
    match an.tokens.get(next).map(|t| &t.token) {
        Some(Token::ArrowFunction) => Some("function".into()),
        Some(Token::Op(op)) => match op {
            Operator::Concat => Some("string".into()),
            Operator::Equal
            | Operator::NotEqual
            | Operator::Less
            | Operator::LessEqual
            | Operator::Greater
            | Operator::GreaterEqual => Some("boolean".into()),
            Operator::Add
            | Operator::Sub
            | Operator::Multiply
            | Operator::Divide
            | Operator::FloorDivide
            | Operator::Modulus
            | Operator::Exponent => ty.or_else(|| {
                primary_type(an, next + 1, uri, lib, depth).and_then(|(t, _)| t)
            }),
            _ => ty,
        },
        _ => ty,
    }
}

/// Index just past the bracket group opening at `i` (`(`, `[` or `{`).
fn skip_group(an: &Analysis, i: usize) -> usize {
    let mut depth = 0i32;
    for (j, t) in an.tokens.iter().enumerate().skip(i) {
        match t.token {
            Token::OpenParen | Token::OpenBracket | Token::OpenBrace => depth += 1,
            Token::CloseParen | Token::CloseBracket | Token::CloseBrace => {
                depth -= 1;
                if depth == 0 {
                    return j + 1;
                }
            }
            _ => {}
        }
    }
    an.tokens.len()
}

/// Type of the primary expression (literal, or name/parens followed by `.f`, `:m(…)`,
/// `(…)`, `[…]`) at token `i`, and the index after it. The type is `None` when unknown.
fn primary_type(
    an: &Analysis,
    i: usize,
    uri: &str,
    lib: &Library,
    depth: u8,
) -> Option<(Option<String>, usize)> {
    let tok = an.tokens.get(i)?;
    let lit = |t: &str| Some((Some(t.to_string()), i + 1));
    let mut ty: Option<String>;
    let mut j;
    match &tok.token {
        Token::Number(_) | Token::Integer(_) => return lit("number"),
        Token::StringLiteral(_) => return lit("string"),
        Token::True | Token::False => return lit("boolean"),
        Token::Nil => return Some((None, i + 1)),
        Token::Function => return lit("function"),
        Token::OpenBrace => return Some((Some("table".into()), skip_group(an, i))),
        Token::Op(Operator::Sub) => return primary_type(an, i + 1, uri, lib, depth),
        Token::Op(Operator::Length) => {
            let (_, next) = primary_type(an, i + 1, uri, lib, depth)?;
            return Some((Some("number".into()), next));
        }
        Token::Op(Operator::Not) => {
            let (_, next) = primary_type(an, i + 1, uri, lib, depth)?;
            return Some((Some("boolean".into()), next));
        }
        Token::OpenParen => {
            ty = expr_type(an, i + 1, uri, lib, depth);
            j = skip_group(an, i);
        }
        Token::Identifier(_) => {
            ty = match lookup(an, i, uri, lib) {
                Some(Found::Local(sym)) => match an.symbols.iter().position(|s| std::ptr::eq(s, sym)) {
                    Some(s) => infer_symbol(an, s, uri, lib, depth),
                    None => sym.ty.clone(),
                },
                Some(Found::Lib(_, sym)) => annotated(sym).ty,
                _ => None,
            };
            j = i + 1;
        }
        _ => return None,
    }
    // the callee of a following call: the name at `callee`
    let mut callee = matches!(tok.token, Token::Identifier(_)).then_some(i);
    loop {
        match an.tokens.get(j).map(|t| &t.token) {
            Some(Token::Dot | Token::Colon)
                if matches!(an.tokens.get(j + 1).map(|t| &t.token), Some(Token::Identifier(_))) =>
            {
                callee = Some(j + 1);
                // a method keeps the receiver's type to resolve a generic `vec` return
                if !matches!(an.tokens[j].token, Token::Colon) {
                    ty = None;
                }
                j += 2;
            }
            Some(Token::OpenParen | Token::StringLiteral(_) | Token::OpenBrace) => {
                let receiver = ty.take();
                ty = callee.and_then(|c| match lookup(an, c, uri, lib)? {
                    Found::Local(sym) | Found::Lib(_, sym) => annotated(sym).returns,
                    Found::Builtin(b) if b.returns == "vec" => receiver.or(Some("vec".into())),
                    Found::Builtin(b) => Some(b.returns.to_string()),
                    Found::Stdlib(_) => None,
                });
                callee = None;
                j = match an.tokens[j].token {
                    Token::StringLiteral(_) => j + 1,
                    _ => skip_group(an, j),
                };
            }
            Some(Token::OpenBracket) => {
                ty = None;
                callee = None;
                j = skip_group(an, j);
            }
            _ => break,
        }
    }
    Some((ty, j))
}

// ─────────────────────────────── libraries ────────────────────────────────

/// Definitions other files contribute to every document: a `---@meta` stub (e.g.
/// Petrichor64's generated `ignore.lua`) or a sibling script sharing the same
/// globals. Hover, completion, signature help and go-to-definition consult it
/// for names a document doesn't define itself. Files are keyed by path, so a
/// `file://` URI and its plain path name the same file.
#[derive(Debug, Default, Clone)]
pub struct Library {
    files: BTreeMap<String, Vec<Symbol>>,
    /// Per file: checker declarations from its annotated globals (see [`Library::declarations`]).
    decls: BTreeMap<String, Vec<(String, String)>>,
}

impl Library {
    /// (Re)load `source` as library file `uri`, replacing what it held before.
    pub fn add(&mut self, uri: &str, source: &str) {
        let symbols = analyze(source)
            .symbols
            .into_iter()
            .filter(|s| match s.kind {
                SymbolKind::Function
                | SymbolKind::Global
                | SymbolKind::Field
                | SymbolKind::Method => true,
                SymbolKind::TypeAlias => s.scope == 0,
                _ => false,
            })
            .collect::<Vec<_>>();
        let decls = symbols.iter().filter_map(declaration_of).collect();
        self.decls.insert(uri_to_path(uri), decls);
        self.files.insert(uri_to_path(uri), symbols);
    }

    pub fn remove(&mut self, uri: &str) {
        self.files.remove(&uri_to_path(uri));
        self.decls.remove(&uri_to_path(uri));
    }

    /// Type declarations for the checker (`Compiler::declare`), from the annotated
    /// globals of every file but `uri`: a function's `---@param`/`---@return` tags (or
    /// silt annotations) as `(T, U) -> R`, a `---@type` global as its type. With these
    /// a call like `sin("x")` or `local s: string = sin(1)` is a type error.
    pub fn declarations(&self, uri: &str) -> Vec<(String, String)> {
        let own = uri_to_path(uri);
        let mut seen = std::collections::BTreeSet::new();
        self.decls
            .iter()
            .filter(|(path, _)| **path != own)
            .flat_map(|(_, d)| d.iter())
            .filter(|(name, _)| seen.insert(name.clone()))
            .cloned()
            .collect()
    }

    pub fn contains(&self, uri: &str) -> bool {
        self.files.contains_key(&uri_to_path(uri))
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every library symbol outside document `uri` (whose own analysis is fresher).
    fn others<'a>(&'a self, uri: &str) -> impl Iterator<Item = (&'a str, &'a Symbol)> {
        let own = uri_to_path(uri);
        self.files
            .iter()
            .filter(move |(path, _)| **path != own)
            .flat_map(|(path, syms)| syms.iter().map(move |s| (path.as_str(), s)))
    }

    /// Non-member definitions visible to `uri`, one per name (a documented one wins).
    fn globals<'a>(&'a self, uri: &str) -> Vec<(&'a str, &'a Symbol)> {
        let mut out: BTreeMap<&str, (&str, &Symbol)> = BTreeMap::new();
        for (path, sym) in self.others(uri) {
            if matches!(sym.kind, SymbolKind::Field | SymbolKind::Method) {
                continue;
            }
            match out.get(sym.name.as_str()) {
                Some((_, prev)) if prev.doc.is_some() || sym.doc.is_none() => {}
                _ => {
                    out.insert(&sym.name, (path, sym));
                }
            }
        }
        out.into_values().collect()
    }

    fn global<'a>(&'a self, uri: &str, name: &str) -> Option<(&'a str, &'a Symbol)> {
        self.globals(uri).into_iter().find(|(_, s)| s.name == name)
    }

    /// `function recv.name` / `recv:name` definitions on receiver `recv`.
    fn members<'a>(&'a self, uri: &str, recv: &str) -> Vec<(&'a str, &'a Symbol)> {
        self.others(uri)
            .filter(|(_, s)| {
                matches!(s.kind, SymbolKind::Field | SymbolKind::Method)
                    && s.receiver.as_deref() == Some(recv)
            })
            .collect()
    }
}

/// The checker declaration for a library global, if its annotations say anything: a
/// function with at least one typed parameter or return, or a typed global. Pieces in
/// lua-language-server spelling are translated; any the checker can't parse become `any`.
fn declaration_of(sym: &Symbol) -> Option<(String, String)> {
    let sym = annotated(sym);
    match sym.kind {
        SymbolKind::Function => {
            let params = split_params(sym.params.as_deref().unwrap_or(""));
            let returns = split_params(sym.returns.as_deref().unwrap_or(""));
            let typed = params.iter().any(|p| p.contains(':')) || !returns.is_empty();
            if !typed {
                return None;
            }
            let params: Vec<String> = params
                .iter()
                .map(|p| {
                    let ty = p.split_once(':').map(|(_, t)| checked_type(t));
                    match (p.trim().starts_with("..."), ty) {
                        (true, Some(t)) => format!("...{}", t),
                        (true, None) => "...any".into(),
                        (false, Some(t)) => t,
                        (false, None) => "any".into(),
                    }
                })
                .collect();
            let returns: Vec<String> = returns.iter().map(|r| checked_type(r)).collect();
            let ret = match returns.len() {
                0 => "any".to_string(),
                1 => returns[0].clone(),
                _ => format!("({})", returns.join(", ")),
            };
            Some((sym.name.clone(), format!("({}) -> {}", params.join(", "), ret)))
        }
        SymbolKind::Global => sym.ty.as_deref().map(|t| (sym.name.clone(), checked_type(t))),
        _ => None,
    }
}

/// `ty` translated to silt annotation syntax, or `any` when the checker can't parse it.
fn checked_type(ty: &str) -> String {
    let t = silt_type(ty);
    #[cfg(feature = "typing")]
    {
        let mut probe = Compiler::new_with_flags(true, true, false);
        if probe.declare("probe", &t).is_err() {
            return "any".into();
        }
    }
    t
}

/// Split `text` on `sep` at bracket depth 0.
fn split_top(text: &str, sep: char) -> Vec<&str> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

/// A lua-language-server type in silt annotation syntax: `integer` → `number`,
/// `T[]` → `{T}`, `fun(a: T): R` → `(T) -> R`, shape field `k?: T` → `k: T?`, a
/// string-literal type → `string`. Anything else passes through.
fn silt_type(ty: &str) -> String {
    let parts: Vec<String> = split_top(ty.trim(), '|').into_iter().map(silt_atom).collect();
    parts.join(" | ")
}

fn silt_atom(t: &str) -> String {
    let t = t.trim();
    // a whole-type `?` (`number?`, `{T}?`)
    if let Some(inner) = t.strip_suffix('?') {
        let inner = silt_type(inner);
        return if inner.contains('|') { format!("({})?", inner) } else { format!("{}?", inner) };
    }
    // array suffix `T[]` / `T[3]`
    if t.ends_with(']') {
        if let Some(open) = t.rfind('[') {
            let size = &t[open + 1..t.len() - 1];
            if open > 0 && size.chars().all(|c| c.is_ascii_digit()) {
                return format!("{{{}}}", silt_type(&t[..open]));
            }
        }
    }
    if let Some(rest) = t.strip_prefix("fun(") {
        let mut depth = 1i32;
        let close = rest.char_indices().find_map(|(i, c)| {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        });
        if let Some(close) = close {
            let params: Vec<String> = split_top(&rest[..close], ',')
                .into_iter()
                .filter(|p| !p.trim().is_empty())
                .map(|p| match p.split_once(':') {
                    Some((n, ty)) if n.trim().ends_with('?') => format!("{}?", silt_type(ty)),
                    Some((_, ty)) => silt_type(ty),
                    None if p.trim() == "..." => "...any".into(),
                    None => "any".into(),
                })
                .collect();
            let ret = rest[close + 1..].trim().trim_start_matches(':').trim();
            let ret = if ret.is_empty() { "any".to_string() } else { silt_type(ret) };
            return format!("({}) -> {}", params.join(", "), ret);
        }
    }
    if let Some(inner) = t.strip_prefix('{').and_then(|r| r.strip_suffix('}')) {
        let fields = split_top(inner, ',');
        let shape = fields.iter().any(|f| split_top(f, ':').len() > 1);
        if !shape {
            return format!("{{{}}}", silt_type(inner));
        }
        let fields: Vec<String> = fields
            .into_iter()
            .filter(|f| !f.trim().is_empty())
            .map(|f| match f.split_once(':') {
                Some((k, ty)) => match k.trim().strip_suffix('?') {
                    Some(k) => format!("{}: {}?", k, silt_type(ty)),
                    None => format!("{}: {}", k.trim(), silt_type(ty)),
                },
                None => f.trim().to_string(),
            })
            .collect();
        return format!("{{{}}}", fields.join(", "));
    }
    if let Some(inner) = t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
        return format!("({})", silt_type(inner));
    }
    match t {
        "integer" => "number".into(),
        _ if t.starts_with('\'') || t.starts_with('"') => "string".into(),
        _ => t.to_string(),
    }
}

/// `file:///a%20b/c.lua` → `/a b/c.lua`; anything else is returned unchanged.
pub fn uri_to_path(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else {
        return uri.to_string();
    };
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `/a b/c.lua` → `file:///a%20b/c.lua` (RFC 3986, as editors encode them); a
/// non-path id is returned unchanged.
pub fn path_to_uri(path: &str) -> String {
    if !path.starts_with('/') {
        return path.to_string();
    }
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// A native the runtime registers, with the signature and docs the stdlib's
/// name listing can't provide.
pub struct Builtin {
    /// `vec3`, or `vec.dot` for a method of the `vec` library.
    pub name: &'static str,
    /// Called with `:` (the receiver is the implicit first argument).
    pub method: bool,
    pub params: &'static str,
    pub returns: &'static str,
    pub doc: &'static str,
}

impl Builtin {
    fn describe(&self) -> String {
        let head = match self.name.split_once('.') {
            Some((lib, m)) if self.method => format!("{}:{}", lib, m),
            _ => self.name.to_string(),
        };
        format!(
            "```lua\nfunction {}({}): {}\n```\n\n{}",
            head, self.params, self.returns, self.doc
        )
    }
}

#[cfg(feature = "vector")]
pub const BUILTINS: &[Builtin] = &[
    Builtin { name: "vec2", method: false, params: "x: number, y: number", returns: "vec2", doc: "Immutable 2D vector (f32). Fields `.x .y`; supports `+ - * /` and unary `-`." },
    Builtin { name: "vec3", method: false, params: "x: number, y: number, z: number", returns: "vec3", doc: "Immutable 3D vector (f32). Fields `.x .y .z`; supports `+ - * /` and unary `-`." },
    Builtin { name: "vec4", method: false, params: "x: number, y: number, z: number, w: number", returns: "vec4", doc: "Immutable 4D vector (f32). Fields `.x .y .z .w`; supports `+ - * /` and unary `-`." },
    Builtin { name: "vec.length", method: true, params: "", returns: "number", doc: "Euclidean length, any dimension." },
    Builtin { name: "vec.length_squared", method: true, params: "", returns: "number", doc: "Length squared; cheaper than `length` (no sqrt)." },
    Builtin { name: "vec.normalize", method: true, params: "", returns: "vec", doc: "Unit vector in the same direction, as a new vector." },
    Builtin { name: "vec.dot", method: true, params: "other: vec", returns: "number", doc: "Dot product; both operands must be the same dimension." },
    Builtin { name: "vec.distance", method: true, params: "other: vec", returns: "number", doc: "Euclidean distance; both operands must be the same dimension." },
    Builtin { name: "vec.cross", method: true, params: "other: vec3", returns: "vec3", doc: "Cross product; 3D only." },
];
#[cfg(not(feature = "vector"))]
pub const BUILTINS: &[Builtin] = &[];

fn builtin(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

/// What an undeclared name at a token resolves to, outside the document's own symbols.
enum Found<'a> {
    Local(&'a Symbol),
    /// A library file's path and its definition.
    Lib(&'a str, &'a Symbol),
    Builtin(&'static Builtin),
    /// A standard-library name with no further docs.
    Stdlib(String),
}

/// The `recv.` / `recv:` before token `i`, if it is a member access: (`.` or `:`, receiver).
fn member_access(an: &Analysis, i: usize) -> Option<(char, Option<String>)> {
    let op = match an.tokens.get(i.checked_sub(1)?)?.token {
        Token::Dot => '.',
        Token::Colon => ':',
        _ => return None,
    };
    let recv = i
        .checked_sub(2)
        .and_then(|j| match &an.tokens[j].token {
            Token::Identifier(n) => Some(n.to_string()),
            _ => None,
        });
    Some((op, recv))
}

/// Resolve the identifier at token `i`: the document's own symbol, else a library
/// definition, a documented native or a stdlib name — as a member when it follows
/// `recv.` / `recv:` (an unknown receiver's `:` method may be a vector or string one).
fn lookup<'a>(an: &'a Analysis, i: usize, uri: &str, lib: &'a Library) -> Option<Found<'a>> {
    let tok = an.tokens.get(i)?;
    if let Some(s) = an.symbol_at(tok.range.start) {
        return Some(Found::Local(&an.symbols[s]));
    }
    let name = match &tok.token {
        Token::Identifier(n) => n.to_string(),
        _ => return None,
    };
    match member_access(an, i) {
        None => {
            // a global defined later in the file (a use only resolves to earlier ones)
            if let Some(sym) = an.symbols.iter().find(|s| {
                s.name == name && matches!(s.kind, SymbolKind::Function | SymbolKind::Global)
            }) {
                return Some(Found::Local(sym));
            }
            if let Some((path, sym)) = lib.global(uri, &name) {
                return Some(Found::Lib(path, sym));
            }
            if let Some(b) = builtin(&name) {
                return Some(Found::Builtin(b));
            }
            stdlib_names()
                .contains(&name)
                .then(|| Found::Stdlib(name))
        }
        Some((op, recv)) => {
            if let Some(recv) = &recv {
                if let Some((path, sym)) = lib.members(uri, recv).into_iter().find(|(_, s)| s.name == name) {
                    return Some(Found::Lib(path, sym));
                }
                let qualified = format!("{}.{}", recv, name);
                if let Some(b) = builtin(&qualified) {
                    return Some(Found::Builtin(b));
                }
                if stdlib_names().contains(&qualified) {
                    return Some(Found::Stdlib(qualified));
                }
            }
            if op != ':' {
                return None;
            }
            if let Some(b) = builtin(&format!("vec.{}", name)).filter(|b| b.method) {
                return Some(Found::Builtin(b));
            }
            let s = format!("string.{}", name);
            stdlib_names().contains(&s).then(|| Found::Stdlib(s))
        }
    }
}

/// `sym` with its doc comment's `---@param` / `---@return` / `---@type` tags (the
/// lua-language-server annotation style) folded into its signature; other tags
/// (`@meta`, `@class`, `@diagnostic`, …) don't describe the symbol and are dropped.
fn annotated(sym: &Symbol) -> Symbol {
    let mut s = sym.clone();
    let Some(doc) = &sym.doc else { return s };
    let mut text: Vec<String> = vec![];
    let mut param_types: Vec<(String, String)> = vec![];
    let mut param_docs: Vec<String> = vec![];
    let mut returns: Vec<String> = vec![];
    for line in doc.lines() {
        let Some(tag) = line.trim().strip_prefix('@') else {
            text.push(line.to_string());
            continue;
        };
        let (kw, rest) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        let (lead_ty, lead_desc) = split_annotation_type(rest);
        match kw {
            "param" => {
                let (raw, after) = rest.trim().split_once(char::is_whitespace).unwrap_or((rest.trim(), ""));
                let (ty, desc) = split_annotation_type(after);
                let ty = ty.to_string();
                let name = raw.trim_end_matches('?');
                let ty = if raw.ends_with('?') { format!("{}?", ty) } else { ty };
                if !desc.is_empty() {
                    param_docs.push(format!("- `{}` — {}", name, desc));
                }
                param_types.push((name.to_string(), ty));
            }
            "return" => {
                if !lead_ty.is_empty() {
                    returns.push(lead_ty.to_string());
                }
            }
            "type" if s.ty.is_none() => {
                s.ty = (!lead_ty.is_empty()).then(|| lead_ty.to_string());
                if !lead_desc.is_empty() {
                    text.push(lead_desc.to_string());
                }
            }
            _ => {}
        }
    }
    if let Some(params) = &s.params {
        let typed: Vec<String> = split_params(params)
            .into_iter()
            .map(|p| match param_types.iter().find(|(n, t)| *n == p && !t.is_empty()) {
                Some((_, t)) if !p.contains(':') => format!("{}: {}", p, t),
                _ => p,
            })
            .collect();
        s.params = Some(typed.join(", "));
    }
    if s.returns.is_none() && !returns.is_empty() {
        s.returns = Some(returns.join(", "));
    }
    let mut body = text.join("\n").trim().to_string();
    if !param_docs.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str(&param_docs.join("\n"));
    }
    s.doc = (!body.is_empty()).then_some(body);
    s
}

/// Split an annotation's leading type from its description: `{ a: number } desc` →
/// (`{ a: number }`, `desc`). Brackets keep their contents whole, a `|` joins the
/// words around it (`string | nil`) and a trailing `:` the next (`fun(): R`).
fn split_annotation_type(text: &str) -> (&str, &str) {
    let text = text.trim();
    let mut depth = 0i32;
    let mut end = text.len();
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '{' | '(' | '[' | '<' => depth += 1,
            '}' | ')' | ']' | '>' => depth -= 1,
            c if c.is_whitespace() && depth <= 0 => {
                let next = text[i..].trim_start();
                // `A | B` and a function type's `fun(…): R` continue past the space
                let joined = next.starts_with('|') || text[..i].ends_with(['|', ':']);
                if !joined {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    (&text[..end], text[end..].trim())
}

fn describe(sym: &Symbol) -> String {
    let sym = &annotated(sym);
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
        SymbolKind::Global => format!("(global) {}{}", sym.name, annot(sym)),
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

/// [`definition`] for document `uri`, reaching into `lib`: the defining file's
/// URI (`uri` itself for a local definition) and range.
pub fn definition_in(source: &str, uri: &str, pos: Position, lib: &Library) -> Option<(String, Range)> {
    let an = analyze(source);
    if let Some(i) = an.symbol_at(pos) {
        return Some((uri.to_string(), an.symbols[i].range));
    }
    match lookup(&an, an.token_at(pos)?, uri, lib)? {
        Found::Local(sym) => Some((uri.to_string(), sym.range)),
        Found::Lib(path, sym) => Some((path_to_uri(path), sym.range)),
        _ => None,
    }
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
    completion_in(source, "", pos, &Library::default())
}

/// [`completion`] for document `uri`, also offering `lib`'s globals and members
/// and (after `:`) the documented vector methods.
pub fn completion_in(source: &str, uri: &str, pos: Position, lib: &Library) -> Vec<Completion> {
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
        for (_, sym) in lib.members(uri, recv) {
            push(&mut out, &sym.name, CompletionKind::Function, Some(describe_short(sym)));
        }
        let lib_prefix = format!("{}.", recv);
        for n in stdlib_names() {
            if let Some(member) = n.strip_prefix(&lib_prefix) {
                push(&mut out, member, CompletionKind::Function, Some(n.clone()));
            }
        }
        if lead.ends_with(':') {
            for b in BUILTINS.iter().filter(|b| b.method) {
                let m = b.name.rsplit('.').next().unwrap_or(b.name);
                push(&mut out, m, CompletionKind::Function, Some(format!("vec:{}({}): {}", m, b.params, b.returns)));
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
    for (_, sym) in lib.globals(uri) {
        let kind = match sym.kind {
            SymbolKind::Function => CompletionKind::Function,
            SymbolKind::TypeAlias => CompletionKind::Type,
            _ => CompletionKind::Variable,
        };
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
    let sym = &annotated(sym);
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
    signature_help_in(source, "", pos, &Library::default())
}

/// [`signature_help`] for document `uri`, for callees defined in `lib` or
/// documented natives too.
pub fn signature_help_in(
    source: &str,
    uri: &str,
    pos: Position,
    lib: &Library,
) -> Option<SignatureHelp> {
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
    let params_text = match lookup(&an, open - 1, uri, lib)? {
        Found::Local(sym) | Found::Lib(_, sym) => annotated(sym).params?,
        Found::Builtin(b) => b.params.to_string(),
        Found::Stdlib(_) => return None,
    };
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
        completion_in, definition_in, diagnostics_with, document_symbols, format_with, hover_in,
        references, rename, semantic_tokens, signature_help_in, uri_to_path,
        CompletionKind, Diagnostic, Library, Position, Range, Severity, SymbolKind,
        SEMANTIC_TOKEN_TYPES,
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

    /// `.lua` / `.silt` source files.
    fn is_script(path: &std::path::Path) -> bool {
        matches!(path.extension().and_then(|e| e.to_str()), Some("lua" | "silt"))
    }

    /// Scripts under `dir`, recursively, skipping hidden and build directories; capped
    /// so a library pointed at a huge tree (or `$HOME`) can't stall startup.
    fn collect_scripts(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        const MAX_FILES: usize = 500;
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            if out.len() >= MAX_FILES {
                return;
            }
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    collect_scripts(&path, out);
                }
            } else if is_script(&path) {
                out.push(path);
            }
        }
    }

    fn load_library_file(library: &mut Library, path: &std::path::Path) {
        const MAX_BYTES: u64 = 1 << 20;
        if std::fs::metadata(path).map(|m| m.len() > MAX_BYTES).unwrap_or(true) {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(path) {
            library.add(&path.to_string_lossy(), &text);
        }
    }

    /// Whether document `uri` belongs to the library: already loaded, or a script
    /// inside a library directory (e.g. a newly created file).
    fn in_library(library: &Library, dirs: &[String], uri: &str) -> bool {
        let path = uri_to_path(uri);
        library.contains(uri)
            || (is_script(std::path::Path::new(&path)) && dirs.iter().any(|d| path.starts_with(d)))
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
        // Host type declarations from `initializationOptions`:
        //   { "declare": { "instr": "(number, snippet(number, number) -> number) -> nil" },
        //     "declareInSnippet": { "emit": "(number) -> nil" } }
        // (name, type, also available inside snippet VMs)
        let mut declarations: Vec<(String, String, bool)> = Vec::new();
        // Library files from `initializationOptions.library` (files or directories):
        //   { "library": ["/path/to/project"] }
        // Their globals and doc comments feed hover/completion/definition in every doc.
        let mut library = Library::default();
        let mut library_dirs: Vec<String> = Vec::new();
        let diagnose = |text: &str, declarations: &[(String, String, bool)], library_decls: Vec<(String, String)>| {
            diagnostics_with(text, |compiler| {
                // the library's annotated globals first, so explicit host declarations win
                for (name, ty) in library_decls {
                    if let Err(e) = compiler.declare(&name, &ty) {
                        eprintln!("silt lsp: ignoring library declaration of `{}`: {}", name, e);
                    }
                }
                for (name, ty, in_snippet) in declarations {
                    let r = if *in_snippet {
                        compiler.declare_in_snippet(name, ty)
                    } else {
                        compiler.declare(name, ty)
                    };
                    if let Err(e) = r {
                        eprintln!("silt lsp: ignoring declaration of `{}`: {}", name, e);
                    }
                }
            })
        };

        while let Some(msg) = read_message(&mut reader) {
            let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
            let id = msg.get("id").cloned();

            match method {
                "initialize" => {
                    let opts = &msg["params"]["initializationOptions"];
                    for (key, in_snippet) in [("declare", false), ("declareInSnippet", true)] {
                        if let Some(map) = opts[key].as_object() {
                            for (name, ty) in map {
                                if let Some(ty) = ty.as_str() {
                                    declarations.push((name.clone(), ty.to_string(), in_snippet));
                                }
                            }
                        }
                    }
                    for entry in opts["library"].as_array().into_iter().flatten() {
                        let Some(path) = entry.as_str().map(uri_to_path) else { continue };
                        let p = std::path::Path::new(&path);
                        if p.is_dir() {
                            library_dirs.push(format!("{}/", path.trim_end_matches('/')));
                            let mut files = vec![];
                            collect_scripts(p, &mut files);
                            for f in files {
                                load_library_file(&mut library, &f);
                            }
                        } else {
                            load_library_file(&mut library, p);
                        }
                    }
                    if !library.is_empty() {
                        eprintln!("silt lsp: loaded {} library file(s)", library.len());
                    }
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
                        let diags = diagnose(text, &declarations, library.declarations(&uri));
                        if in_library(&library, &library_dirs, &uri) {
                            library.add(&uri, text);
                        }
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
                        let diags = diagnose(text, &declarations, library.declarations(&uri));
                        if in_library(&library, &library_dirs, &uri) {
                            library.add(&uri, text);
                        }
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
                    if library.contains(&uri) {
                        // unsaved edits are gone; the file on disk is the library again
                        library.remove(&uri);
                        load_library_file(&mut library, std::path::Path::new(&uri_to_path(&uri)));
                    }
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
                        .and_then(|text| hover_in(text, &uri, pos, &library))
                        .map(|md| json!({ "contents": { "kind": "markdown", "value": md } }))
                        .unwrap_or(Value::Null);
                    send_response(&mut writer, id.unwrap_or(Value::Null), result);
                }
                "textDocument/definition" => {
                    let (uri, pos) = doc_position(&msg);
                    let result = docs
                        .get(&uri)
                        .and_then(|text| definition_in(text, &uri, pos, &library))
                        .map(|(uri, r)| json!({ "uri": uri, "range": range_json(&r) }))
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
                        .map(|text| completion_in(text, &uri, pos, &library))
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
                        .and_then(|text| signature_help_in(text, &uri, pos, &library))
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
