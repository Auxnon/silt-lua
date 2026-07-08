//! A token that cannot begin an expression — a stray closing `)`, a leading or
//! dangling binary operator — used to be silently dropped by the single-pass
//! parser (its prefix rule was the no-op `void`), so malformed input compiled
//! clean and never produced a diagnostic. `parse_precedence` now reports these as
//! `InvalidTokenPlacement`, which is what the CLI and the LSP surface.
//!
//! Fixing that exposed a latent coupling: `build_function` never consumed the `)`
//! closing a parameter list — it relied on the body's first (phantom) statement to
//! swallow it, which also emitted the placeholder leading POP the call convention
//! depends on and cleared `local_declare_mode`. `build_function` now does all three
//! explicitly, so the tests below also guard that ordinary/local/anon/arrow
//! functions still compile and run correctly.

use silt_lua::{simple, ExVal};

/// `simple` returns `ExVal::String("… failed with: …")` when compilation fails.
fn error_of(src: &str) -> Option<String> {
    match simple(src) {
        ExVal::String(s) if s.contains("failed with:") => Some(s),
        _ => None,
    }
}

fn assert_invalid_token(src: &str) {
    match error_of(src) {
        Some(msg) => assert!(
            msg.contains("Invalid token placement"),
            "expected an `Invalid token placement` error for {src:?}, got: {msg}"
        ),
        None => panic!("expected {src:?} to be rejected, but it compiled clean"),
    }
}

// ---- newly-detected errors -------------------------------------------------

#[test]
fn stray_close_paren_after_value() {
    assert_invalid_token("local x = 1)");
}

#[test]
fn extra_close_paren_in_return() {
    assert_invalid_token("return (1 + 2))");
}

#[test]
fn close_paren_where_expression_expected() {
    assert_invalid_token("local x = )");
}

#[test]
fn dangling_binary_operator() {
    assert_invalid_token("local x = 1 +");
}

#[test]
fn leading_binary_operator() {
    assert_invalid_token("return * 2");
}

// ---- `local`/`global` with no identifier ----------------------------------
// These used to `todo!()` (a panic that would take down the LSP server) or, for a
// bare `local` at EOF, silently produce a chunk that ran garbage.

fn assert_expected_local_ident(src: &str) {
    match error_of(src) {
        Some(msg) => assert!(
            msg.contains("Expected identifier following local keyword"),
            "expected an `ExpectedLocalIdentifier` error for {src:?}, got: {msg}"
        ),
        None => panic!("expected {src:?} to be rejected, but it compiled clean"),
    }
}

#[test]
fn local_followed_by_assign() {
    assert_expected_local_ident("local = 5");
}

#[test]
fn local_followed_by_literal() {
    assert_expected_local_ident("local 5");
}

#[test]
fn global_followed_by_assign() {
    assert_expected_local_ident("global = 5");
}

#[test]
fn bare_local_at_eof_is_rejected() {
    // Degenerate case: `local` with nothing after. The message is generic, but it
    // must be an error rather than a chunk that "succeeds" and returns garbage.
    assert!(
        error_of("local").is_some(),
        "bare `local` should be rejected, not compile clean"
    );
}

// ---- constructs that must still compile & run (no false positives) ---------

#[test]
fn global_function_params_intact() {
    assert_eq!(simple("function f(a, b) return a + b end return f(3, 4)"), ExVal::Integer(7));
}

#[test]
fn local_function_params_intact() {
    // Regression: the leading-POP / local_declare_mode reset must be applied so
    // that a `local function` with multiple params resolves each slot correctly.
    assert_eq!(
        simple("local function f(a, b) return a + b end return f(3, 4)"),
        ExVal::Integer(7),
    );
    assert_eq!(
        simple("local function f(a, b, c) return a end return f(3, 4, 5)"),
        ExVal::Integer(3),
    );
}

#[test]
fn anonymous_function_intact() {
    assert_eq!(simple("return (function(a) return a end)(9)"), ExVal::Integer(9));
}

#[test]
fn member_function_intact() {
    assert_eq!(
        simple("local m = {} function m.f(x) return x end return m.f(4)"),
        ExVal::Integer(4),
    );
}

#[test]
fn arrow_returning_and_capturing_intact() {
    assert_eq!(
        simple("local function adder(n) return x -> x + n end return adder(5)(10)"),
        ExVal::Integer(15),
    );
    assert_eq!(
        simple("local function apply(g, v) return g(v) end return apply(x -> x * 10, 5)"),
        ExVal::Integer(50),
    );
}

#[test]
fn nested_grouping_intact() {
    assert_eq!(simple("return ((1 + 2) * 3)"), ExVal::Integer(9));
}
