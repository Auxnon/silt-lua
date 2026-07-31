//! Lua 5.3/5.4 pattern matching — a faithful port of the matcher in `lstrlib.c`.
//!
//! Lua patterns are **not** regular expressions; they are a smaller, self-contained language
//! (character classes `%a %d %s %w …` and complements, sets `[...]`, anchors `^ $`, the
//! quantifiers `* + - ?`, captures `(...)`, position captures `()`, `%b`, `%f`, and
//! back-references `%1`–`%9`). Everything operates on **bytes**, exactly like reference Lua,
//! so ASCII class semantics match and multibyte input degrades the same way it does in Lua.
//!
//! Callers pass byte slices and get back byte offsets; the `string.*` layer converts those to
//! Lua's 1-based positions and slices the source (via `from_utf8_lossy`, since a `.` can land
//! mid-codepoint).

use crate::error::SiltError;

const L_ESC: u8 = b'%';
const CAP_UNFINISHED: isize = -1;
const CAP_POSITION: isize = -2;
const MAX_CAPTURES: usize = 32;
const MAX_CALLS: usize = 220; // recursion guard (matchdepth in lstrlib)

/// A resolved capture: either a substring (byte range `[start, end)`) or a position capture
/// (`()` in the pattern), which yields a 1-based index.
#[derive(Debug, Clone)]
pub enum Capture {
    Str(usize, usize),
    Pos(usize),
}

/// A successful match: byte range of the whole match plus its explicit captures (empty if the
/// pattern had none — the caller substitutes the whole match in that case).
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub start: usize,
    pub end: usize,
    pub caps: Vec<Capture>,
}

struct MatchState<'a> {
    src: &'a [u8],
    pat: &'a [u8],
    /// (init, len): len == CAP_UNFINISHED while open, CAP_POSITION for `()`, else byte length.
    caps: Vec<(usize, isize)>,
    depth: usize,
}

fn err(msg: &str) -> SiltError {
    SiltError::Custom(msg.to_string())
}

/// C `isspace`: space, \t, \n, \v, \f, \r (note \v = 0x0b, which Rust's is_ascii_whitespace omits).
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Match a single byte against a class letter (`a d l s u w c p x g`); uppercase = complement.
fn match_class(c: u8, cl: u8) -> bool {
    let res = match cl.to_ascii_lowercase() {
        b'a' => c.is_ascii_alphabetic(),
        b'd' => c.is_ascii_digit(),
        b'l' => c.is_ascii_lowercase(),
        b's' => is_space(c),
        b'u' => c.is_ascii_uppercase(),
        b'w' => c.is_ascii_alphanumeric(),
        b'c' => c.is_ascii_control(),
        b'p' => c.is_ascii_punctuation(),
        b'x' => c.is_ascii_hexdigit(),
        b'g' => c.is_ascii_graphic(), // printable except space
        // Not a class letter: `%` before a magic/normal char → literal match of that char.
        _ => return cl == c,
    };
    if cl.is_ascii_uppercase() {
        !res
    } else {
        res
    }
}

impl<'a> MatchState<'a> {
    /// Given `p` at the start of a single pattern item (a class), return the index just past it.
    fn class_end(&self, mut p: usize) -> Result<usize, SiltError> {
        let pat = self.pat;
        let c = pat[p];
        p += 1;
        if c == L_ESC {
            if p >= pat.len() {
                return Err(err("malformed pattern (ends with '%')"));
            }
            return Ok(p + 1);
        }
        if c == b'[' {
            if pat.get(p) == Some(&b'^') {
                p += 1;
            }
            // do-while: consume one member unconditionally (so a `]` right after `[`/`[^` is a
            // literal member, e.g. `[]]`), then stop once the *next* char is the closing `]`.
            loop {
                if p >= pat.len() {
                    return Err(err("malformed pattern (missing ']')"));
                }
                let cc = pat[p];
                p += 1;
                if cc == L_ESC && p < pat.len() {
                    p += 1; // skip escaped char inside the set
                }
                if pat.get(p) == Some(&b']') {
                    return Ok(p + 1);
                }
            }
        }
        Ok(p)
    }

    /// Does `src[s]` match the class occupying `pat[p..ep]`?
    fn single_match(&self, s: usize, p: usize, ep: usize) -> bool {
        if s >= self.src.len() {
            return false;
        }
        let c = self.src[s];
        match self.pat[p] {
            b'.' => true,
            L_ESC => match_class(c, self.pat[p + 1]),
            b'[' => match_bracket_class(c, self.pat, p, ep - 1),
            other => other == c,
        }
    }

    fn match_balance(&mut self, s: usize, p: usize) -> Result<Option<usize>, SiltError> {
        if p + 1 >= self.pat.len() {
            return Err(err("missing arguments to '%b'"));
        }
        if s >= self.src.len() || self.src[s] != self.pat[p] {
            return Ok(None);
        }
        let b = self.pat[p];
        let e = self.pat[p + 1];
        let mut cont = 1i32;
        let mut s = s + 1;
        while s < self.src.len() {
            if self.src[s] == e {
                cont -= 1;
                if cont == 0 {
                    return Ok(Some(s + 1));
                }
            } else if self.src[s] == b {
                cont += 1;
            }
            s += 1;
        }
        Ok(None)
    }

    fn max_expand(&mut self, s: usize, p: usize, ep: usize) -> Result<Option<usize>, SiltError> {
        let mut i = 0;
        while self.single_match(s + i, p, ep) {
            i += 1;
        }
        loop {
            if let Some(r) = self.do_match(s + i, ep + 1)? {
                return Ok(Some(r));
            }
            if i == 0 {
                return Ok(None);
            }
            i -= 1;
        }
    }

    fn min_expand(&mut self, mut s: usize, p: usize, ep: usize) -> Result<Option<usize>, SiltError> {
        loop {
            if let Some(r) = self.do_match(s, ep + 1)? {
                return Ok(Some(r));
            }
            if self.single_match(s, p, ep) {
                s += 1;
            } else {
                return Ok(None);
            }
        }
    }

    fn start_capture(
        &mut self,
        s: usize,
        p: usize,
        what: isize,
    ) -> Result<Option<usize>, SiltError> {
        if self.caps.len() >= MAX_CAPTURES {
            return Err(err("too many captures"));
        }
        self.caps.push((s, what));
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.caps.pop();
        }
        Ok(r)
    }

    fn end_capture(&mut self, s: usize, p: usize) -> Result<Option<usize>, SiltError> {
        // close the most recent still-open capture
        let l = match self.caps.iter().rposition(|c| c.1 == CAP_UNFINISHED) {
            Some(l) => l,
            None => return Err(err("invalid pattern capture")),
        };
        self.caps[l].1 = (s - self.caps[l].0) as isize;
        let r = self.do_match(s, p)?;
        if r.is_none() {
            self.caps[l].1 = CAP_UNFINISHED;
        }
        Ok(r)
    }

    fn match_capture(&mut self, s: usize, idx: u8) -> Result<Option<usize>, SiltError> {
        let l = (idx - b'1') as usize;
        if l >= self.caps.len() || self.caps[l].1 == CAP_UNFINISHED {
            return Err(err("invalid capture index"));
        }
        let (init, len) = self.caps[l];
        let len = len as usize;
        if self.src.len() - s >= len && self.src[init..init + len] == self.src[s..s + len] {
            Ok(Some(s + len))
        } else {
            Ok(None)
        }
    }

    /// The core recursive matcher: try to match `pat[p..]` against `src[s..]`; return the byte
    /// index just past the match, or `None`.
    fn do_match(&mut self, mut s: usize, mut p: usize) -> Result<Option<usize>, SiltError> {
        self.depth += 1;
        if self.depth > MAX_CALLS {
            self.depth -= 1;
            return Err(err("pattern too complex"));
        }
        let result = loop {
            if p >= self.pat.len() {
                break Ok(Some(s));
            }
            match self.pat[p] {
                b'(' => {
                    break if self.pat.get(p + 1) == Some(&b')') {
                        self.start_capture(s, p + 2, CAP_POSITION)
                    } else {
                        self.start_capture(s, p + 1, CAP_UNFINISHED)
                    };
                }
                b')' => break self.end_capture(s, p + 1),
                b'$' if p + 1 == self.pat.len() => {
                    break Ok(if s == self.src.len() { Some(s) } else { None });
                }
                L_ESC => match self.pat.get(p + 1) {
                    Some(b'b') => match self.match_balance(s, p + 2)? {
                        Some(ns) => {
                            s = ns;
                            p += 4;
                            continue;
                        }
                        None => break Ok(None),
                    },
                    Some(b'f') => {
                        p += 2;
                        if self.pat.get(p) != Some(&b'[') {
                            break Err(err("missing '[' after '%f' in pattern"));
                        }
                        let ep = self.class_end(p)?;
                        let prev = if s == 0 { 0 } else { self.src[s - 1] };
                        let cur = if s < self.src.len() { self.src[s] } else { 0 };
                        if !match_bracket_class(prev, self.pat, p, ep - 1)
                            && match_bracket_class(cur, self.pat, p, ep - 1)
                        {
                            p = ep;
                            continue;
                        }
                        break Ok(None);
                    }
                    Some(d) if d.is_ascii_digit() => match self.match_capture(s, *d)? {
                        Some(ns) => {
                            s = ns;
                            p += 2;
                            continue;
                        }
                        None => break Ok(None),
                    },
                    _ => break self.default_match(s, p),
                },
                _ => break self.default_match(s, p),
            }
        };
        self.depth -= 1;
        result
    }

    /// A single pattern class plus optional suffix (`* + - ?` or none).
    fn default_match(&mut self, s: usize, p: usize) -> Result<Option<usize>, SiltError> {
        let ep = self.class_end(p)?;
        let matched = self.single_match(s, p, ep);
        let suffix = self.pat.get(ep).copied();
        if !matched {
            // can the class be skipped entirely?
            if matches!(suffix, Some(b'*') | Some(b'?') | Some(b'-')) {
                return self.do_match(s, ep + 1);
            }
            return Ok(None); // '+' or no suffix → fail
        }
        match suffix {
            Some(b'?') => {
                if let Some(r) = self.do_match(s + 1, ep + 1)? {
                    Ok(Some(r))
                } else {
                    self.do_match(s, ep + 1)
                }
            }
            Some(b'+') => self.max_expand(s + 1, p, ep),
            Some(b'*') => self.max_expand(s, p, ep),
            Some(b'-') => self.min_expand(s, p, ep),
            _ => self.do_match(s + 1, ep),
        }
    }

    /// Convert the internal `(init, len)` captures into resolved `Capture`s (1-based positions
    /// for `()`, byte ranges otherwise), clamped to the source bounds.
    fn resolve_caps(&self) -> Vec<Capture> {
        self.caps
            .iter()
            .map(|&(init, len)| {
                if len == CAP_POSITION {
                    Capture::Pos(init + 1)
                } else {
                    let l = if len < 0 { 0 } else { len as usize };
                    Capture::Str(init, (init + l).min(self.src.len()))
                }
            })
            .collect()
    }
}

/// `[...]` set membership. `p` points at `'['`; `ec` points at the closing `']'`.
fn match_bracket_class(c: u8, pat: &[u8], p: usize, ec: usize) -> bool {
    let mut sig = true;
    let mut p = p + 1;
    if pat.get(p) == Some(&b'^') {
        sig = false;
        p += 1;
    }
    while p < ec {
        if pat[p] == L_ESC {
            p += 1;
            if match_class(c, pat[p]) {
                return sig;
            }
            p += 1;
        } else if pat.get(p + 1) == Some(&b'-') && p + 2 < ec {
            if pat[p] <= c && c <= pat[p + 2] {
                return sig;
            }
            p += 3;
        } else if pat[p] == c {
            return sig;
        } else {
            p += 1;
        }
    }
    !sig
}

/// Try to match `pat` anchored exactly at byte `s` (no forward search). A leading `^` in the
/// pattern is skipped here since the anchor is implied by the fixed position — this is what
/// `gsub` steps with.
pub fn match_at(src: &[u8], pat: &[u8], s: usize) -> Result<Option<MatchResult>, SiltError> {
    let p_start = if pat.first() == Some(&b'^') { 1 } else { 0 };
    let mut ms = MatchState {
        src,
        pat,
        caps: Vec::new(),
        depth: 0,
    };
    match ms.do_match(s, p_start)? {
        Some(e) => Ok(Some(MatchResult {
            start: s,
            end: e,
            caps: ms.resolve_caps(),
        })),
        None => Ok(None),
    }
}

/// Search `src[init..]` for the first match of `pat`. Honors a leading `^` anchor (which pins the
/// match to `init` and disables the forward scan).
pub fn find(src: &[u8], pat: &[u8], init: usize) -> Result<Option<MatchResult>, SiltError> {
    let anchor = pat.first() == Some(&b'^');
    let mut s = init.min(src.len());
    loop {
        if let Some(m) = match_at(src, pat, s)? {
            return Ok(Some(m));
        }
        if anchor || s >= src.len() {
            return Ok(None);
        }
        s += 1;
    }
}
