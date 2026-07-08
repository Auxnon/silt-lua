//! mlua-style native multi-return (PLAN §6.1 / value.rs `ToLuaMulti`).
//!
//! A native function's success type `R` implements `ToLuaMulti`:
//! - a **tuple** `(A, B, …)` spreads into multiple Lua values,
//! - a **scalar** is a single value,
//! - a **collection** (`Vec`, `[T; N]`) is one *table* (it does NOT spread).
//!
//! `math.modf` is the canonical typed tuple-return native fn (`R = (f64, f64)`).

use silt_lua::{simple, ExVal};

#[test]
fn tuple_return_spreads_to_multiple_values() {
    // math.modf(3.75) -> (3.0, 0.75) as TWO values.
    assert_eq!(
        simple("local i, f = math.modf(3.75) return i"),
        ExVal::Number(3.0)
    );
    assert_eq!(
        simple("local i, f = math.modf(3.75) return f"),
        ExVal::Number(0.75)
    );
}

#[test]
fn tuple_return_in_single_context_takes_first() {
    // Assigned to one target, only the integral part is kept.
    assert_eq!(simple("local i = math.modf(9.25) return i"), ExVal::Number(9.0));
}

#[test]
fn tuple_return_negative_fraction() {
    assert_eq!(
        simple("local i, f = math.modf(-2.5) return f"),
        ExVal::Number(-0.5)
    );
}

#[test]
fn both_values_usable_together() {
    // Reassemble the number from its two returned parts.
    assert_eq!(
        simple("local i, f = math.modf(6.5) return i + f"),
        ExVal::Number(6.5)
    );
}

#[test]
fn scalar_native_return_is_single() {
    // math.floor returns one value (sanity that the single path still works).
    assert_eq!(simple("return math.floor(3.9)"), ExVal::Integer(3));
}
