// Arrow functions (cargo feature `arrow`, on by default). `params -> body`,
// where a single param may be bare and multiple params are parenthesized. The
// body is a single expression or a `do … end` block, and is ALWAYS implicitly
// returned (regardless of the implicit-return language flag).
#![cfg(feature = "arrow")]
use silt_lua::{simple, valeq, ExVal};

#[test]
fn single_param() {
    valeq!("local f = x -> x + 1 return f(5)", ExVal::Integer(6));
    valeq!("local sq = n -> n * n return sq(7)", ExVal::Integer(49));
}

#[test]
fn parenthesized_params() {
    valeq!("local add = (a, b) -> a + b return add(3, 4)", ExVal::Integer(7));
    valeq!("local f = (a, b) -> a return f(3, 4)", ExVal::Integer(3));
    valeq!("local f = (a, b) -> b return f(3, 4)", ExVal::Integer(4));
    valeq!("local f = (a, b, c) -> a*100 + b*10 + c return f(1, 2, 3)", ExVal::Integer(123));
    valeq!("local id = (x) -> x return id(42)", ExVal::Integer(42));
}

#[test]
fn do_block_body() {
    valeq!(
        "local f = x -> do local y = x * 2 return y + 1 end return f(10)",
        ExVal::Integer(21)
    );
    valeq!(
        "local f = (a, b) -> do local s = a + b s = s * 2 return s end return f(3, 4)",
        ExVal::Integer(14)
    );
}

#[test]
fn arrow_as_argument() {
    // a trailing comma ends the arrow body; `5` is apply's second argument
    valeq!(
        "local function apply(g, v) return g(v) end return apply(x -> x * 10, 5)",
        ExVal::Integer(50)
    );
    valeq!(
        "local function apply2(g, a, b) return g(a, b) end return apply2((x, y) -> x * y, 6, 7)",
        ExVal::Integer(42)
    );
}

#[test]
fn closure_capture() {
    valeq!(
        "local function adder(n) return x -> x + n end return adder(5)(10)",
        ExVal::Integer(15)
    );
}

#[test]
fn currying() {
    valeq!("local g = x -> y -> x + y return g(3)(4)", ExVal::Integer(7));
}

#[test]
fn iife() {
    valeq!("return ((x) -> x * x)(9)", ExVal::Integer(81));
}

#[test]
fn zero_param() {
    valeq!("local f = () -> 42 return f()", ExVal::Integer(42));
    valeq!("local g = () -> do return 7 end return g()", ExVal::Integer(7));
}

// Typed arrow parameters require both `arrow` and `typing`. The annotations are
// parsed (and currently discarded — tracking on the param local is a follow-up);
// the function runs with the usual dynamic semantics.
#[cfg(feature = "typing")]
#[test]
fn typed_params() {
    valeq!("local f = (a: number, b: number) -> a + b return f(3, 4)", ExVal::Integer(7));
    valeq!("local f = (x: number) -> x * 2 return f(21)", ExVal::Integer(42));
    valeq!("local f = (a: number, b: string) -> a return f(5, 'hi')", ExVal::Integer(5));
}

// With `typing` on, `(a: …` is normally a typed param list — but a parenthesized
// method call `(obj:method(…))` is disambiguated by the `(` after the name and
// recovered as an ordinary grouped expression.
#[cfg(feature = "typing")]
#[test]
fn parenthesized_method_call_not_mistaken_for_typed_param() {
    valeq!(
        "local t = {x = 5} function t:get() return self.x end return (t:get())",
        ExVal::Integer(5)
    );
    // a trailing operator inside the parens is handled too
    valeq!(
        "local t = {x = 5} function t:get() return self.x end return (t:get() + 1)",
        ExVal::Integer(6)
    );
    valeq!(
        "local t = {x = 5} function t:add(n) return self.x + n end return (t:add(10)) * 2",
        ExVal::Integer(30)
    );
}

// `(a:m()) -> …` is contradictory (we committed to a method call) and must error.
#[cfg(feature = "typing")]
#[test]
fn arrow_after_grouped_method_call_errors() {
    let out = simple(
        "local t = {} function t:m() return 1 end local g = (t:m()) -> 1 return g",
    );
    match out {
        ExVal::String(s) => assert!(!s.is_empty(), "expected an error message"),
        other => panic!("expected an error, got {:?}", other),
    }
}

#[test]
fn implicit_return_even_without_flag() {
    // arrows implicitly return their single-expression body
    valeq!("local double = n -> n + n return double(21)", ExVal::Integer(42));
}

#[test]
fn grouping_still_works() {
    // the grouping path must not mistake plain parens for arrow params
    valeq!("return (1 + 2) * 3", ExVal::Integer(9));
    valeq!("local a = 5 return (a) + 1", ExVal::Integer(6));
    valeq!("local a = 2 local b = 3 return (a + b) * 2", ExVal::Integer(10));
}
