//! Userdata metamethod dispatch beyond `__pairs` (PLAN §3 / §6.1). The built-in
//! `test_ent()` (`TestEnt`) registers `__tostring` and `__concat` (both return
//! `"[entity 0]"`), plus `__pairs`. These exercise the call sites that route userdata
//! operands through `call_meta_method`.

use silt_lua::{simple, ExVal};

#[test]
fn tostring_metamethod() {
    assert_eq!(
        simple("local e = test_ent() return tostring(e)"),
        ExVal::String("[entity 0]".to_string())
    );
}

#[test]
fn concat_metamethod_userdata_left() {
    // `ud .. x` dispatches __concat (TestEnt's returns a fixed string).
    assert_eq!(
        simple(r#"local e = test_ent() return e .. "!""#),
        ExVal::String("[entity 0]".to_string())
    );
}

#[test]
fn concat_metamethod_userdata_right() {
    // `x .. ud` also reaches __concat (userdata on the right, checked before the string
    // fallback).
    assert_eq!(
        simple(r#"local e = test_ent() return "x=" .. e"#),
        ExVal::String("[entity 0]".to_string())
    );
}

#[test]
fn comparison_metamethods() {
    // TestEnt defines __eq (always true), __lt (always false), __le (always true).
    // Distinct objects, so these exercise the metamethods (not identity).
    assert_eq!(simple("local a=test_ent() local b=test_ent() return a == b"), ExVal::Bool(true));
    assert_eq!(simple("local a=test_ent() local b=test_ent() return a < b"), ExVal::Bool(false));
    assert_eq!(simple("local a=test_ent() local b=test_ent() return a <= b"), ExVal::Bool(true));
    // `a > b` is evaluated as `b < a` → __lt → false.
    assert_eq!(simple("local a=test_ent() local b=test_ent() return a > b"), ExVal::Bool(false));
}

#[test]
fn equality_same_object_is_identity() {
    // Same object short-circuits to true without consulting __eq.
    assert_eq!(simple("local a=test_ent() return a == a"), ExVal::Bool(true));
}

#[test]
fn arithmetic_metamethod_absent_errors() {
    // TestEnt defines no __add; the binary-op path reports a clean error rather than
    // silently coercing (confirms userdata arithmetic routes to the metamethod path).
    match simple("local e = test_ent() return e + 1") {
        ExVal::String(s) => assert!(
            s.contains("failed with:"),
            "expected an error for `ud + 1` without __add, got: {s}"
        ),
        other => panic!("expected an error, got {other:?}"),
    }
}
