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
