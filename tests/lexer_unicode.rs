//! Regression: the lexer indexes `source` (a &str) by BYTE offset when slicing
//! out token text, but tracked `current`/`start_token` as CHARACTER counts. A
//! single multi-byte UTF-8 char (e.g. an em-dash in a string or comment) shifted
//! byte-vs-char, so every following token was mis-sliced — a later number
//! literal came out garbled and raised a spurious `NotANumber`, and error
//! columns reported an absolute offset instead of a per-line column.
//! See lexer.rs `eat`/`eat_out` (advance `current` by `len_utf8`) and `_error`.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp)
        .map_err(|e| e.to_string())
        .unwrap()
}

#[test]
fn number_after_multibyte_string_lexes() {
    // The em-dash (3 bytes, 1 char) precedes the float; pre-fix `12.5` was
    // mis-sliced and failed to parse.
    assert_eq!(run("local s = \"a — b\"\nreturn 12.5"), ExVal::Number(12.5));
}

#[test]
fn number_after_multibyte_comment_lexes() {
    assert_eq!(
        run("-- note: em—dash here\nreturn 40.0 * 0.5"),
        ExVal::Number(20.0)
    );
}

#[test]
fn arithmetic_after_multibyte_is_not_spurious_nan() {
    // Mirrors the petrichor payload that surfaced this: a string with an em-dash
    // above a float multiply. Pre-fix this raised NotANumber("* 0.0").
    let src = "local msg = \"killed — vanish\"\nlocal dx = 3.0\nreturn dx * 0.005";
    assert_eq!(run(src), ExVal::Number(3.0 * 0.005));
}

#[test]
fn multiple_multibyte_chars_stay_aligned() {
    // Several multi-byte chars compound the drift if bytes/chars are conflated.
    assert_eq!(
        run("local a = \"→ ★ — ✓\"\nreturn 7.0 + 1.0"),
        ExVal::Number(8.0)
    );
}
