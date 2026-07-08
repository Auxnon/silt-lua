//! Validates that errors are reported at the expected source line, and exercises the
//! `error_snippet` helper (which turns a stored source + an error location into a
//! caret-annotated snippet — usable after the fact, off-thread).

use silt_lua::error::{ErrorOut, ErrorTuple};
use silt_lua::{error_snippet, Compiler, Lua};

/// Compile+run `src`, expecting failure; return the whole `ErrorOut`.
fn err_out(src: &str) -> ErrorOut {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(None, src, &mut compiler) {
        Ok(v) => panic!("expected an error, got {:?}", v),
        Err(out) => out,
    }
}

fn first(src: &str) -> ErrorTuple {
    err_out(src).errors.into_iter().next().expect("no errors")
}

/// 1-indexed line of the first error.
fn err_line(src: &str) -> usize {
    first(src).location.0
}

// ── error line validation ────────────────────────────────────────────────────

#[test]
fn runtime_error_lines() {
    // Each program's error sits on the line noted; the leading lines just push it down.
    assert_eq!(err_line("local a=1\nlocal b=2\nreturn nil + 1"), 3);
    assert_eq!(err_line("local a=1\nreturn x()"), 2); // call a nil
    assert_eq!(err_line("local a=1\nreturn x.field"), 2); // index a nil
    assert_eq!(err_line("local a=1\nlocal b=2\nlocal c=3\nreturn {} .. 1"), 4);
    assert_eq!(err_line("local s=0\nfor i=1,nil do s=s+i end return s"), 2);
    assert_eq!(err_line("local a=1\nlocal b=2\nlocal c=3\nlocal d=4\nreturn nil+1"), 5);
}

#[test]
fn compile_error_line() {
    // Unterminated `if … then` block opened on line 3.
    assert_eq!(err_line("local a=1\nlocal b=2\nif x then"), 3);
}

#[test]
fn error_in_nested_function_body_line() {
    let src = "local function f()\n  local x = 1\n  return nil + 1\nend\nreturn f()";
    assert_eq!(err_line(src), 3);
}

// ── snippet helper ───────────────────────────────────────────────────────────

#[test]
fn snippet_points_at_the_column() {
    // line 2 = "def", col 2 → caret under 'e'
    assert_eq!(error_snippet("abc\ndef\nghi", (2, 2)), "2 | def\n  |  ^");
}

#[test]
fn snippet_preserves_tabs_for_alignment() {
    // Leading tab is echoed into the caret indent so the caret still lines up.
    assert_eq!(error_snippet("\tx=)", (1, 3)), "1 | \tx=)\n  | \t ^");
}

#[test]
fn snippet_out_of_range_is_graceful() {
    assert_eq!(error_snippet("abc", (5, 1)), "  (line 5 not in source)");
    assert_eq!(error_snippet("abc", (0, 1)), "  (line 0 not in source)");
}

#[test]
fn snippet_from_a_real_error() {
    // The whole flow: keep the source, run, then render a snippet from the raised error.
    let src = "local a = 1\nlocal b = 2\nreturn nil + 1";
    let out = err_out(src);
    let rendered = out.snippet(src);
    // Points at line 3 and shows that line's text with a caret and the message.
    assert!(rendered.contains("return nil + 1"), "snippet:\n{rendered}");
    assert!(rendered.contains('^'), "snippet:\n{rendered}");
    assert!(rendered.contains("line 3"), "snippet:\n{rendered}");
    // And the single-error ErrorTuple helper agrees.
    let one = first(src).snippet(src);
    assert!(one.contains("return nil + 1") && one.contains('^'));
}
