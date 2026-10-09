//! Validates that errors are reported at the expected source line, and exercises the
//! `error_snippet` helper (which turns a stored source + an error location into a
//! caret-annotated snippet — usable after the fact, off-thread).

use silt_lua::error::{ErrorOut, ErrorTuple};
use silt_lua::{error_snippet, error_span_snippet, Compiler, Lua};

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

/// The location comes from the faulting instruction, not the value-stack
/// height. It used to index `locations` by stack depth, so a table
/// constructor with several fields (leaving a deep stack) dragged the
/// reported line back into the constructor.
#[test]
fn error_after_wide_constructor_line() {
    assert_eq!(err_line("local b = { x = 1, y = 2 }\nb.ent.x = 1"), 2);
    let src = "function f()\n  local b = { x = 1, y = 2, z = 3 }\n  b.ent = 5\n  b.ent.x = 1\nend\nf()";
    assert_eq!(err_line(src), 4);
    let src = "function spawn(x, y, z, dx, dy)\n  local b = { x = x, y = y, z = z, dx = dx, dy = dy, age = 0, ent = nil }\n  local c = nil\n  c.flipped = true\nend\nspawn(1, 2, 3, 4, 5)";
    assert_eq!(err_line(src), 4);
}

/// Columns after line 1 are exact: the lexer used to reset the column before
/// eating the `\n`, so line 2+ columns were one too far right.
#[test]
fn columns_after_first_line() {
    assert_eq!(first("local a = 1\nundefined_fn(1, 2)").location, (2, 1));
    assert_eq!(first("local a = 1\nlocal b = a + nil").location, (2, 11));
}

/// A runtime error spans the whole failing expression or statement.
#[test]
fn runtime_error_span() {
    let e = first("local a = 1\nlocal b = a + nil");
    assert_eq!((e.location, e.end), ((2, 11), Some((2, 17))));
    let e = first("local b = {}\nb.ent = 5\nb.ent.flipped = true");
    assert_eq!((e.location, e.end), ((3, 1), Some((3, 20))));
    let src = "local t = {}\nprint(t.a.b.c)";
    assert_eq!(
        err_out(src).snippet(src),
        "error: Cannot perform table operations on a non-table value (nil) (line 2, col 7)\n\
         2 | print(t.a.b.c)\n  |       ^^^^^^^"
    );
}

#[test]
fn span_snippet_underlines_start_to_end() {
    assert_eq!(
        error_span_snippet("abc\n\tb.ent = 1\nghi", (2, 2), (2, 6)),
        "2 | \tb.ent = 1\n  | \t^^^^^"
    );
    // multi-line: each line's covered part, later lines from their first non-blank
    assert_eq!(
        error_span_snippet("x = 1 +\n  2", (1, 5), (2, 3)),
        "1 | x = 1 +\n  |     ^^^\n2 |   2\n  |   ^"
    );
    // end before start falls back to a single caret
    assert_eq!(error_span_snippet("abc", (1, 2), (1, 1)), error_snippet("abc", (1, 2)));
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
