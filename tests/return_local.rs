//! `return x` for a local compiles to RETURN_LOCAL, and a single value returned
//! to a single-value caller takes RETURN's fast path. These cover the callers
//! and code shapes around both.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).unwrap_or_else(|e| panic!("{src}: {e:?}"))
}

#[test]
fn returns_a_param_and_a_local() {
    let src = "
        local function id(n) return n end
        local function sq(n) local r = n * n return r end
        return id(4) + sq(5)";
    assert_eq!(run(src), ExVal::Integer(29));
}

#[test]
fn short_circuit_jumps_to_the_plain_return() {
    // `a and x`: a false `a` jumps past the patched GET_LOCAL to the RETURN
    let src = "
        local function f(a, x) return a and x end
        local function g(a, x) return a or x end
        local r = {}
        r[1] = f(false, 2) == false
        r[2] = f(true, 2) == 2
        r[3] = g(nil, 3) == 3
        r[4] = g(1, 3) == 1
        return r[1] and r[2] and r[3] and r[4]";
    assert_eq!(run(src), ExVal::Bool(true));
}

#[test]
fn padded_and_spread_callers() {
    let src = "
        local function one(n) return n end
        local a, b, c = one(7)
        local function sum(...) local s = 0 for _, v in ipairs({...}) do s = s + v end return s end
        local t = { one(1), one(2) }
        return a * 1000 + (b == nil and 100 or 0) + (c == nil and 10 or 0) + sum(one(5)) + #t";
    assert_eq!(run(src), ExVal::Integer(7000 + 100 + 10 + 5 + 2));
}

#[test]
fn under_pcall_the_callee_is_the_root_frame() {
    let src = "
        local function f(n) local m = n + 1 return m end
        local ok, v = pcall(f, 41)
        return ok and v";
    assert_eq!(run(src), ExVal::Integer(42));
}

#[test]
fn returned_local_captured_by_a_closure() {
    // upvalues over the frame close before its slots are reused
    let src = "
        local function make(n)
            local x = n
            local get = function() return x end
            x = x + 1
            return get
        end
        local g1 = make(10)
        local g2 = make(20)
        local function keep(v) local y = v return y end
        return keep(g1()) + keep(g2())";
    assert_eq!(run(src), ExVal::Integer(11 + 21));
}

#[test]
fn recursion_returns_locals() {
    let src = "
        local function fib(n)
            if n < 2 then return n end
            local a = fib(n - 1)
            local b = fib(n - 2)
            local s = a + b
            return s
        end
        return fib(20)";
    assert_eq!(run(src), ExVal::Integer(6765));
}
