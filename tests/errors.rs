// Programs that must raise a runtime error rather than silently producing a value.
use silt_lua::{Compiler, Lua};

/// Run a program and return Ok(value-display) or Err(first-error-message).
fn run_err(source: &str) -> Result<String, String> {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    lua.run(None, source, &mut compiler)
        .map(|v| v.to_string())
        .map_err(|e| e.to_string())
}

#[test]
fn arithmetic_on_nil_errors() {
    let e = run_err("return nil + 1").expect_err("nil + 1 must raise");
    assert!(e.to_lowercase().contains("nil"), "unexpected error: {e}");
}

#[test]
fn add_word_string_to_number_errors() {
    // Numeric strings coerce; non-numeric strings must raise.
    let e = run_err("return 'a1' + 2").expect_err("'a1' + 2 must raise");
    assert!(!e.is_empty(), "expected a type error, got: {e}");
}

#[test]
fn call_non_function_errors() {
    let e = run_err("local x = 5 return x()").expect_err("calling a number must raise");
    assert!(!e.is_empty(), "unexpected: {e}");
}

#[test]
fn index_non_table_errors() {
    let e = run_err("local x = 5 return x.field").expect_err("indexing a number must raise");
    assert!(!e.is_empty(), "unexpected: {e}");
}

#[test]
fn string_coercion_in_arithmetic_succeeds() {
    // Sanity counter-case: numeric strings DO coerce.
    assert_eq!(run_err("return '5' + 2"), Ok("7".to_string()));
}

#[test]
fn bitwise_on_non_integer_errors() {
    // A float with a fractional part has no integer representation (Lua).
    let e = run_err("return 1.5 & 1").expect_err("bitwise on non-integer must raise");
    assert!(!e.is_empty(), "unexpected: {e}");
}
