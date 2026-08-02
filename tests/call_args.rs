use silt_lua::{simple, valeq, ExVal};

// SILT-BUGS.md #1: a call argument that is itself a call, followed by local arguments,
// used to invoke the wrong callee ("... is not callable"). Root cause: parsing the nested
// call recursed into `arguments()`, which reset `can_multivar_set`/`arg_mode` to fixed
// defaults on exit instead of restoring the caller's — so a following local started a
// multivar comma-walk and swallowed the next argument, losing one and mis-resolving the
// callee. Fixed by save/restoring that parsing context in `arguments()`/`call_table`/
// `call_string`.

#[test]
fn nested_call_arg_then_locals() {
    valeq!(
        r#"
        function text(a,b,c,d) return a..b..c..d end
        function flr(x) return x end
        local y = 10
        local col = "DDE"
        return text("lit", flr(8), y, col)
        "#,
        ExVal::String("lit810DDE".to_string())
    );
}

#[test]
fn nested_call_arg_then_locals_method_syntax() {
    valeq!(
        r#"
        local gui = {}
        function gui.text(self, a, b, c, d) return a..b..c..d end
        function flr(x) return x end
        local y = 10
        local col = "Z"
        return gui:text("m", flr(8), y, col)
        "#,
        ExVal::String("m810Z".to_string())
    );
}

#[test]
fn two_nested_calls_then_global() {
    valeq!(
        r#"
        function text(a,b,c,d) return a..b..c..d end
        function flr(x) return x end
        local y = 3
        return text("a", flr(8), flr(y), "G")
        "#,
        ExVal::String("a83G".to_string())
    );
}

// The multiret behaviour the detection is actually for must still hold: a *trailing* call
// spreads its results; a non-trailing one (or one wrapped in parens/arithmetic) does not.

#[test]
fn trailing_call_spreads_results() {
    valeq!(
        "local function g() return 2,3 end local function add(a,b) return a+b end return add(g())",
        ExVal::Integer(5)
    );
    valeq!(
        "local function g() return 1,2,3 end local function sum(a,b,c) return a+b+c end return sum(g())",
        ExVal::Integer(6)
    );
}

#[test]
fn non_trailing_call_is_truncated() {
    // g() is not the last argument, so it contributes exactly one value
    valeq!(
        "local function g() return 2,3 end local function add(a,b) return a+b end return add(g(), 100)",
        ExVal::Integer(102)
    );
}

#[test]
fn call_wrapped_arg_does_not_spread() {
    // a table constructor / grouping / arithmetic argument that merely CONTAINS a call
    // must not be mistaken for a trailing call
    valeq!("local function id(x) return x end local function g() return 7 end return id({g()})[1]", ExVal::Integer(7));
    valeq!("local function id(x) return x end local function g() return 7 end return id((g()))", ExVal::Integer(7));
    valeq!("local function two(a,b) return a+b end local function g() return 3 end return two(g()+1, 10)", ExVal::Integer(14));
}

#[test]
fn pcall_with_erroring_closure_literal() {
    // regression guard: a function-literal argument whose body ends in a call
    // (`error(...)`) must not make pcall treat the literal as a trailing multiret arg
    valeq!("local ok = pcall(function() error('boom') end) return ok", ExVal::Bool(false));
    valeq!("local ok,e = pcall(function() error('boom') end) return e", ExVal::String("boom".to_string()));
}
