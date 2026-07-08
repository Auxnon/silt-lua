//! Tests for the pure-Rust LSP analysis core (feature `lsp`): compile diagnostics
//! and Lua-style re-indentation. No JSON/transport here.
#![cfg(feature = "lsp")]

use silt_lua::lsp::{diagnostics, format, format_with, Severity};

#[test]
fn clean_source_has_no_diagnostics() {
    assert!(diagnostics("local x = 5\nreturn x").is_empty());
    assert!(diagnostics("").is_empty());
    assert!(diagnostics("function f(a, b) return a + b end").is_empty());
}

#[test]
fn syntax_error_is_reported() {
    // An unterminated `if … then` block is a compile error silt detects.
    let diags = diagnostics("if x then");
    assert!(!diags.is_empty(), "expected at least one diagnostic");
    assert_eq!(diags[0].severity, Severity::Error);
    assert!(!diags[0].message.is_empty());
    // 0-indexed positions
    assert_eq!(diags[0].range.start.line, 0);
}

#[test]
fn error_line_is_zero_indexed() {
    // Unterminated block opened on the third source line → 0-indexed line 2.
    let src = "local a = 1\nlocal b = 2\nif x then";
    let diags = diagnostics(src);
    assert!(!diags.is_empty());
    assert_eq!(diags[0].range.start.line, 2);
}

#[test]
fn format_indents_a_function_body() {
    assert_eq!(
        format("function f()\nreturn 1\nend"),
        "function f()\n\treturn 1\nend"
    );
}

#[test]
fn format_handles_if_elseif_else() {
    let input = "if a then\nx = 1\nelseif b then\nx = 2\nelse\nx = 3\nend";
    let expected = "if a then\n\tx = 1\nelseif b then\n\tx = 2\nelse\n\tx = 3\nend";
    assert_eq!(format(input), expected);
}

#[test]
fn format_handles_nested_blocks_and_repeat() {
    let input = "for i = 1, 3 do\nrepeat\nx = x + 1\nuntil x > 5\nend";
    let expected = "for i = 1, 3 do\n\trepeat\n\t\tx = x + 1\n\tuntil x > 5\nend";
    assert_eq!(format(input), expected);
}

#[test]
fn format_indents_multiline_tables() {
    let input = "local t = {\na = 1,\nb = 2,\n}";
    let expected = "local t = {\n\ta = 1,\n\tb = 2,\n}";
    assert_eq!(format(input), expected);
}

#[test]
fn format_with_spaces() {
    assert_eq!(
        format_with("function f()\nreturn 1\nend", "  "),
        "function f()\n  return 1\nend"
    );
}

#[test]
fn format_is_idempotent() {
    let messy = "function f()\n   if a then\nreturn 1\n      end\nend";
    let once = format(messy);
    assert_eq!(format(&once), once, "formatting twice should be stable");
}

#[test]
fn format_preserves_blank_lines() {
    let input = "local a = 1\n\nlocal b = 2";
    assert_eq!(format(input), "local a = 1\n\nlocal b = 2");
}
