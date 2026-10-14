//! Token-driven scope and symbol analysis for editor features.
//!
//! The compiler is single-pass and emits bytecode as it parses, so there is no
//! AST to walk. For hover / go-to-definition / references / completion we
//! instead run a lightweight pass over the token stream that tracks:
//!
//! * **scopes** — opened by `function`, `do`, `then`, `repeat`, `for`, `while`
//!   (at their `do`), closed by `end` / `until`, with `elseif`/`else` closing
//!   and reopening;
//! * **definitions** — `local x[: T]`, `local function f`, `function f`,
//!   `function a.b` / `a:m`, parameters, loop variables, `type Name = T`,
//!   first assignment to an undeclared global;
//! * **references** — every other identifier, resolved to the innermost
//!   visible definition with that name that precedes it (Lua scoping);
//! * **fields** — `recv.field` / `recv:method` spellings seen per receiver, for
//!   member completion;
//! * **doc comments** — the `--` comment block directly above a definition.
//!
//! It is deliberately tolerant: malformed source still yields whatever symbols
//! can be read, which is what an editor needs while the user is mid-edit.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::TokenTriple;
use crate::lexer::Lexer;
use crate::token::{Operator, Token};

use super::{Position, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Local,
    Parameter,
    Global,
    /// `function f()` at any scope (a global unless `local`).
    Function,
    LocalFunction,
    /// `function t.f()` / `function t:m()` — the last name is the field.
    Field,
    Method,
    LoopVariable,
    TypeAlias,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// The name token.
    pub range: Range,
    /// Scope the symbol is visible in.
    pub scope: usize,
    /// Byte offset of the name token; a reference sees locals defined before it.
    pub offset: usize,
    /// Declared type annotation text (`number`, `{x: number}`), if any.
    pub ty: Option<String>,
    /// For functions: the parameter list text as written, e.g. `a: number, b`.
    pub params: Option<String>,
    /// For functions: declared return type text.
    pub returns: Option<String>,
    /// For `type` aliases: the aliased type's text.
    pub alias_of: Option<String>,
    /// `--` comment block directly above the definition.
    pub doc: Option<String>,
    /// Receiver path for fields/methods (`a.b` for `function a.b.c()`).
    pub receiver: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub range: Range,
    /// Index into `Analysis::symbols`; `None` for an undeclared global.
    pub symbol: Option<usize>,
    /// For `recv.name` / `recv:name` uses: the receiver, so the use can be
    /// matched to a `function recv.name` / `recv:name` definition.
    pub receiver: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub parent: Option<usize>,
    pub start: usize,
    /// Byte offset where the scope closed (`usize::MAX` while open / unterminated).
    pub end: usize,
}

/// One lexed token with its source position, kept for position queries.
#[derive(Debug, Clone)]
pub struct Tok {
    pub token: Token,
    pub range: Range,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub symbols: Vec<Symbol>,
    pub refs: Vec<Reference>,
    pub scopes: Vec<Scope>,
    pub tokens: Vec<Tok>,
    /// receiver name → member names seen as `recv.x` / `recv:x`.
    pub fields: BTreeMap<String, BTreeSet<String>>,
}

pub(super) fn to_range(t: &TokenTriple) -> Range {
    Range {
        start: Position {
            line: t.line.saturating_sub(1) as u32,
            character: t.col.saturating_sub(1) as u32,
        },
        // `end` is the 1-indexed inclusive column of the last char → exclusive 0-indexed
        end: Position {
            line: t.end.0.saturating_sub(1) as u32,
            character: t.end.1 as u32,
        },
    }
}

fn is_ident(t: &Token) -> bool {
    matches!(t, Token::Identifier(_))
}

fn ident_name(t: &Token) -> Option<&str> {
    match t {
        Token::Identifier(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Skip a type expression starting at `i`, returning the index just past it.
/// Grammar mirrors the compiler's type parser: an atom (name, `nil`,
/// `function`, `{...}`, `(...) [-> R]`), then `?` suffixes and `| atom`.
fn skip_type(toks: &[Tok], mut i: usize) -> usize {
    fn skip_atom(toks: &[Tok], mut i: usize) -> usize {
        let Some(t) = toks.get(i) else { return i };
        match &t.token {
            Token::Identifier(_) | Token::Nil | Token::Function => i + 1,
            Token::OpenBrace | Token::OpenParen | Token::OpenBracket => {
                let (open, close) = match &t.token {
                    Token::OpenBrace => (Token::OpenBrace, Token::CloseBrace),
                    Token::OpenParen => (Token::OpenParen, Token::CloseParen),
                    _ => (Token::OpenBracket, Token::CloseBracket),
                };
                let mut depth = 0i32;
                while let Some(tt) = toks.get(i) {
                    if tt.token == open {
                        depth += 1;
                    } else if tt.token == close {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    i += 1;
                }
                // `(T) -> R`
                if matches!(toks.get(i).map(|t| &t.token), Some(Token::ArrowFunction)) {
                    i = skip_type(toks, i + 1);
                }
                i
            }
            _ => i,
        }
    }
    i = skip_atom(toks, i);
    loop {
        match toks.get(i).map(|t| &t.token) {
            Some(Token::Question) => i += 1,
            Some(Token::Op(Operator::BitOr)) => {
                i = skip_atom(toks, i + 1);
            }
            _ => break,
        }
    }
    i
}

fn text_between(source: &str, toks: &[Tok], from: usize, to: usize) -> String {
    if from >= to || from >= toks.len() {
        return String::new();
    }
    let a = toks[from].offset;
    let last = &toks[(to - 1).min(toks.len() - 1)];
    let b = last.offset + last.len;
    source.get(a..b).unwrap_or("").trim().to_string()
}

struct Walker<'s> {
    source: &'s str,
    toks: Vec<Tok>,
    an: Analysis,
    scope_stack: Vec<usize>,
    /// A `for` opened its scope early; the following `do` must not open another.
    for_pending: bool,
    /// Comment lines collected since the last non-comment token, with their line.
    pending_doc: Vec<(u32, String)>,
    /// `{ … }` nesting: an `ident =` inside a constructor is a field key, not an assignment.
    brace_depth: i32,
}

impl<'s> Walker<'s> {
    fn cur_scope(&self) -> usize {
        *self.scope_stack.last().unwrap()
    }

    fn open_scope(&mut self, at: usize) {
        let id = self.an.scopes.len();
        self.an.scopes.push(Scope {
            parent: Some(self.cur_scope()),
            start: at,
            end: usize::MAX,
        });
        self.scope_stack.push(id);
    }

    fn close_scope(&mut self, at: usize) {
        if self.scope_stack.len() > 1 {
            let id = self.scope_stack.pop().unwrap();
            self.an.scopes[id].end = at;
        }
    }

    /// Doc comment for a definition on `line`: the comment block ending on the
    /// line just above it.
    fn take_doc(&mut self, line: u32) -> Option<String> {
        let doc = &self.pending_doc;
        if doc.is_empty() {
            return None;
        }
        let last_line = doc.last().map(|(l, _)| *l).unwrap_or(0);
        if last_line + 1 != line && last_line != line {
            return None;
        }
        // walk back over consecutive lines
        let mut lines: Vec<&str> = vec![];
        let mut expect = last_line;
        for (l, text) in doc.iter().rev() {
            if *l != expect {
                break;
            }
            lines.push(text);
            expect = expect.wrapping_sub(1);
        }
        lines.reverse();
        Some(lines.join("\n"))
    }

    fn define(&mut self, i: usize, kind: SymbolKind) -> Option<usize> {
        let (name, range, offset) = {
            let tok = self.toks.get(i)?;
            (ident_name(&tok.token)?.to_string(), tok.range, tok.offset)
        };
        let scope = self.cur_scope();
        let doc = self.take_doc(range.start.line);
        let id = self.an.symbols.len();
        self.an.symbols.push(Symbol {
            name,
            kind,
            range,
            scope,
            offset,
            ty: None,
            params: None,
            returns: None,
            alias_of: None,
            doc,
            receiver: None,
        });
        Some(id)
    }

    /// Innermost visible definition of `name` at `offset` in the current scope
    /// chain, preferring the latest preceding one.
    fn resolve(&self, name: &str, offset: usize) -> Option<usize> {
        let mut scope = Some(self.cur_scope());
        while let Some(s) = scope {
            let mut best: Option<usize> = None;
            for (i, sym) in self.an.symbols.iter().enumerate() {
                if sym.scope == s && sym.name == name && sym.offset <= offset {
                    if best.map_or(true, |b| self.an.symbols[b].offset < sym.offset) {
                        best = Some(i);
                    }
                }
            }
            if best.is_some() {
                return best;
            }
            scope = self.an.scopes[s].parent;
        }
        // a global defined later in the file (functions are hoisted in practice)
        self.an.symbols.iter().position(|s| {
            s.name == name && matches!(s.kind, SymbolKind::Global | SymbolKind::Function)
        })
    }

    fn reference(&mut self, i: usize) {
        let Some(tok) = self.toks.get(i) else { return };
        let Some(name) = ident_name(&tok.token) else {
            return;
        };
        let name = name.to_string();
        let symbol = self.resolve(&name, tok.offset);
        self.an.refs.push(Reference {
            name,
            range: tok.range,
            symbol,
            receiver: None,
        });
    }

    /// `( params )` at `i` (pointing at `(`): define parameters, return the
    /// index after `)` and the parameter text.
    fn params(&mut self, mut i: usize) -> (usize, String) {
        let start = i + 1;
        if !matches!(self.toks.get(i).map(|t| &t.token), Some(Token::OpenParen)) {
            return (i, String::new());
        }
        i += 1;
        loop {
            match self.toks.get(i).map(|t| t.token.clone()) {
                Some(Token::Identifier(_)) => {
                    let id = self.define(i, SymbolKind::Parameter);
                    i += 1;
                    if matches!(self.toks.get(i).map(|t| &t.token), Some(Token::Colon)) {
                        let ty_start = i + 1;
                        i = skip_type(&self.toks, ty_start);
                        if let Some(id) = id {
                            self.an.symbols[id].ty =
                                Some(text_between(self.source, &self.toks, ty_start, i));
                        }
                    }
                }
                Some(Token::VarArg) => {
                    i += 1;
                    if matches!(self.toks.get(i).map(|t| &t.token), Some(Token::Colon)) {
                        i = skip_type(&self.toks, i + 1);
                    }
                }
                Some(Token::Comma) => i += 1,
                Some(Token::CloseParen) => {
                    let text = text_between(self.source, &self.toks, start, i);
                    return (i + 1, text);
                }
                _ => return (i, text_between(self.source, &self.toks, start, i)),
            }
        }
    }

    fn walk(mut self) -> Analysis {
        let n = self.toks.len();
        let mut i = 0;
        // statement start detection: previous significant token ends a statement
        while i < n {
            let tok = self.toks[i].token.clone();
            let off = self.toks[i].offset;
            match tok {
                Token::Comment => {
                    let t = &self.toks[i];
                    let raw = self.source.get(t.offset..t.offset + t.len).unwrap_or("");
                    let text = raw.trim_start_matches('-').trim();
                    self.pending_doc
                        .push((t.range.start.line, text.to_string()));
                    i += 1;
                    continue;
                }
                Token::Local => {
                    if matches!(
                        self.toks.get(i + 1).map(|t| &t.token),
                        Some(Token::Function)
                    ) {
                        // local function f(params) … end
                        let name_i = i + 2;
                        let id = self.define(name_i, SymbolKind::LocalFunction);
                        self.open_scope(off);
                        let (next, params) = self.params(name_i + 1);
                        let mut j = next;
                        let mut returns = None;
                        if matches!(self.toks.get(j).map(|t| &t.token), Some(Token::Colon)) {
                            let rs = j + 1;
                            j = skip_type(&self.toks, rs);
                            returns = Some(text_between(self.source, &self.toks, rs, j));
                        }
                        if let Some(id) = id {
                            self.an.symbols[id].params = Some(params);
                            self.an.symbols[id].returns = returns;
                        }
                        i = j;
                        continue;
                    }
                    // local a[: T], b[: U] [= …]  — names are defined in the current scope
                    let mut j = i + 1;
                    let mut ids = vec![];
                    loop {
                        if !is_ident(
                            &self
                                .toks
                                .get(j)
                                .map(|t| t.token.clone())
                                .unwrap_or(Token::EOF),
                        ) {
                            break;
                        }
                        ids.push((j, self.define(j, SymbolKind::Local)));
                        j += 1;
                        if matches!(self.toks.get(j).map(|t| &t.token), Some(Token::Colon)) {
                            let ts = j + 1;
                            j = skip_type(&self.toks, ts);
                            if let Some((_, Some(id))) = ids.last() {
                                self.an.symbols[*id].ty =
                                    Some(text_between(self.source, &self.toks, ts, j));
                            }
                        }
                        if matches!(self.toks.get(j).map(|t| &t.token), Some(Token::Comma)) {
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    i = j;
                    continue;
                }
                Token::Function => {
                    // function Name(…) | function a.b.c(…) | function a:m(…) | function(…)
                    let mut j = i + 1;
                    let mut path: Vec<String> = vec![];
                    let mut last_sep_method = false;
                    while let Some(Token::Identifier(name)) =
                        self.toks.get(j).map(|t| t.token.clone())
                    {
                        path.push(name);
                        match self.toks.get(j + 1).map(|t| &t.token) {
                            Some(Token::Dot) => {
                                self.reference_if_first(j, path.len());
                                j += 2;
                            }
                            Some(Token::Colon) => {
                                self.reference_if_first(j, path.len());
                                last_sep_method = true;
                                j += 2;
                            }
                            _ => {
                                j += 1;
                                break;
                            }
                        }
                    }
                    let name_i = if path.is_empty() { None } else { Some(j - 1) };
                    let id = match (name_i, path.len()) {
                        (Some(ni), 1) => self.define(ni, SymbolKind::Function),
                        (Some(ni), _) => {
                            let kind = if last_sep_method {
                                SymbolKind::Method
                            } else {
                                SymbolKind::Field
                            };
                            let id = self.define(ni, kind);
                            if let Some(id) = id {
                                let recv = path[..path.len() - 1].join(".");
                                self.an
                                    .fields
                                    .entry(recv.clone())
                                    .or_default()
                                    .insert(path.last().cloned().unwrap_or_default());
                                self.an.symbols[id].receiver = Some(recv);
                            }
                            id
                        }
                        _ => None,
                    };
                    self.open_scope(off);
                    if last_sep_method {
                        // implicit `self`
                        let scope = self.cur_scope();
                        self.an.symbols.push(Symbol {
                            name: "self".into(),
                            kind: SymbolKind::Parameter,
                            range: self.toks[i].range,
                            scope,
                            offset: off,
                            ty: None,
                            params: None,
                            returns: None,
                            alias_of: None,
                            doc: None,
                            receiver: None,
                        });
                    }
                    let (next, params) = self.params(j);
                    let mut k = next;
                    let mut returns = None;
                    if matches!(self.toks.get(k).map(|t| &t.token), Some(Token::Colon)) {
                        let rs = k + 1;
                        k = skip_type(&self.toks, rs);
                        returns = Some(text_between(self.source, &self.toks, rs, k));
                    }
                    if let Some(id) = id {
                        self.an.symbols[id].params = Some(params);
                        self.an.symbols[id].returns = returns;
                    }
                    i = k;
                    continue;
                }
                Token::For => {
                    // for i = … / for k, v in … do: loop vars live in the loop scope
                    self.open_scope(off);
                    self.for_pending = true;
                    let mut j = i + 1;
                    while let Some(Token::Identifier(_)) = self.toks.get(j).map(|t| &t.token) {
                        self.define(j, SymbolKind::LoopVariable);
                        j += 1;
                        if matches!(self.toks.get(j).map(|t| &t.token), Some(Token::Comma)) {
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    i = j;
                    continue;
                }
                Token::Do => {
                    if self.for_pending {
                        self.for_pending = false;
                    } else {
                        self.open_scope(off);
                    }
                }
                Token::OpenBrace => self.brace_depth += 1,
                Token::CloseBrace => self.brace_depth -= 1,
                Token::Then | Token::Repeat => self.open_scope(off),
                Token::Else | Token::ElseIf => {
                    self.close_scope(off);
                    self.open_scope(off);
                }
                Token::End | Token::Until => self.close_scope(off),
                Token::Identifier(ref name) => {
                    // `type Name = T` alias statement
                    if name == "type"
                        && matches!(
                            self.toks.get(i + 1).map(|t| &t.token),
                            Some(Token::Identifier(_))
                        )
                        && matches!(self.toks.get(i + 2).map(|t| &t.token), Some(Token::Assign))
                    {
                        let id = self.define(i + 1, SymbolKind::TypeAlias);
                        let ts = i + 3;
                        let j = skip_type(&self.toks, ts);
                        if let Some(id) = id {
                            self.an.symbols[id].alias_of =
                                Some(text_between(self.source, &self.toks, ts, j));
                        }
                        i = j;
                        continue;
                    }
                    // member access `recv.x` / `recv:x` — record the field, don't resolve it
                    let after_sep = i > 0
                        && matches!(self.toks[i - 1].token, Token::Dot | Token::Colon)
                        && i >= 2
                        && is_ident(&self.toks[i - 2].token);
                    if after_sep {
                        if let Some(recv) = ident_name(&self.toks[i - 2].token) {
                            let recv = recv.to_string();
                            self.an
                                .fields
                                .entry(recv.clone())
                                .or_default()
                                .insert(name.clone());
                            // resolved against field/method definitions after the walk
                            let range = self.toks[i].range;
                            self.an.refs.push(Reference {
                                name: name.clone(),
                                range,
                                symbol: None,
                                receiver: Some(recv),
                            });
                        }
                        i += 1;
                        continue;
                    }
                    // first assignment to an undeclared name at statement start = global definition
                    let assigned =
                        matches!(self.toks.get(i + 1).map(|t| &t.token), Some(Token::Assign));
                    if self.brace_depth > 0 && assigned {
                        // `{x = 1}`: a constructor key
                        i += 1;
                        continue;
                    }
                    if assigned && self.resolve(name, off).is_none() {
                        self.define(i, SymbolKind::Global);
                    } else {
                        self.reference(i);
                    }
                }
                _ => {}
            }
            if !matches!(tok, Token::Comment) {
                self.pending_doc.clear();
            }
            i += 1;
        }
        // member uses → `function recv.name` / `recv:name` definitions anywhere in the file
        for r in self.an.refs.iter_mut() {
            if r.symbol.is_none() {
                if let Some(recv) = &r.receiver {
                    r.symbol = self.an.symbols.iter().position(|s| {
                        matches!(s.kind, SymbolKind::Field | SymbolKind::Method)
                            && s.name == r.name
                            && s.receiver.as_deref() == Some(recv.as_str())
                    });
                }
            }
        }
        // unterminated scopes run to the end of the file
        let end = self.source.len();
        for s in self.an.scopes.iter_mut() {
            if s.end == usize::MAX {
                s.end = end;
            }
        }
        self.an.tokens = self.toks;
        self.an
    }

    /// `function a.b.c()`: `a` is a reference to the receiver variable.
    fn reference_if_first(&mut self, i: usize, path_len: usize) {
        if path_len == 1 {
            self.reference(i);
        }
    }
}

/// Analyse `source`. Never fails: lex errors end the token stream early and
/// whatever was read so far is returned.
pub fn analyze(source: &str) -> Analysis {
    let mut toks = vec![];
    for r in Lexer::new(source) {
        match r {
            Ok((Token::EOF, _)) => break,
            Ok((token, triple)) => toks.push(Tok {
                token,
                range: to_range(&triple),
                offset: triple.index,
                len: triple.length,
            }),
            Err(_) => break,
        }
    }
    let mut an = Analysis::default();
    an.scopes.push(Scope {
        parent: None,
        start: 0,
        end: usize::MAX,
    });
    let w = Walker {
        source,
        toks,
        an,
        scope_stack: vec![0],
        for_pending: false,
        pending_doc: vec![],
        brace_depth: 0,
    };
    w.walk()
}

impl Analysis {
    pub fn contains(r: &Range, p: Position) -> bool {
        (r.start.line, r.start.character) <= (p.line, p.character)
            && (p.line, p.character) < (r.end.line, r.end.character)
    }

    /// The token under `pos`, or the one ending exactly at it (cursor just
    /// after a word).
    pub fn token_at(&self, pos: Position) -> Option<usize> {
        self.tokens
            .iter()
            .position(|t| Self::contains(&t.range, pos))
            .or_else(|| {
                self.tokens
                    .iter()
                    .position(|t| t.range.end == pos && t.range.start.line == pos.line)
            })
    }

    /// Symbol defined or referenced at `pos`.
    pub fn symbol_at(&self, pos: Position) -> Option<usize> {
        if let Some(i) = self
            .symbols
            .iter()
            .position(|s| Self::contains(&s.range, pos) || s.range.end == pos)
        {
            return Some(i);
        }
        self.refs
            .iter()
            .find(|r| Self::contains(&r.range, pos) || r.range.end == pos)
            .and_then(|r| r.symbol)
    }

    /// Every range naming `symbol`: its definition and all references.
    pub fn occurrences(&self, symbol: usize) -> Vec<Range> {
        let mut out = vec![self.symbols[symbol].range];
        out.extend(
            self.refs
                .iter()
                .filter(|r| r.symbol == Some(symbol))
                .map(|r| r.range),
        );
        out.sort_by_key(|r| (r.start.line, r.start.character));
        out.dedup();
        out
    }

    /// Innermost scope containing byte `offset`.
    pub fn scope_at(&self, offset: usize) -> usize {
        let mut best = 0;
        for (i, s) in self.scopes.iter().enumerate() {
            if s.start <= offset && offset <= s.end && s.start >= self.scopes[best].start {
                best = i;
            }
        }
        best
    }

    /// Symbols visible at `offset`: the scope chain's definitions that precede
    /// it, plus every global/function anywhere in the file.
    pub fn visible_at(&self, offset: usize) -> Vec<&Symbol> {
        let mut out: Vec<&Symbol> = vec![];
        let mut seen = BTreeSet::new();
        let mut scope = Some(self.scope_at(offset));
        while let Some(s) = scope {
            for sym in self.symbols.iter().rev() {
                if sym.scope == s && sym.offset <= offset && seen.insert(sym.name.clone()) {
                    out.push(sym);
                }
            }
            scope = self.scopes[s].parent;
        }
        for sym in &self.symbols {
            if matches!(
                sym.kind,
                SymbolKind::Global | SymbolKind::Function | SymbolKind::TypeAlias
            ) && seen.insert(sym.name.clone())
            {
                out.push(sym);
            }
        }
        out
    }

    /// Byte offset of `pos` (clamped to the source length).
    pub fn offset_of(source: &str, pos: Position) -> usize {
        let mut line = 0u32;
        let mut col = 0u32;
        for (idx, ch) in source.char_indices() {
            if line == pos.line && col == pos.character {
                return idx;
            }
            if ch == '\n' {
                if line == pos.line {
                    return idx;
                }
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        source.len()
    }
}
