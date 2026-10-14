//! `return f(...)` in a function is a tail call: the callee replaces the
//! caller's frame (constant stack depth) and *all* its results pass through,
//! as in Lua. Natives in tail position take the ordinary call path.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}

#[test]
fn tail_recursion_runs_in_constant_stack() {
    // 1e6 iterations: far beyond MAX_STACK if each call kept a frame.
    let src = "
        local function loop(n, acc)
            if n == 0 then return acc end
            return loop(n - 1, acc + 1)
        end
        return loop(1000000, 0)";
    assert_eq!(run(src), Ok(ExVal::Integer(1_000_000)));
}

#[test]
fn mutual_tail_recursion() {
    // Through a table so each function can see the other regardless of
    // definition order (`local a, b` + `function a()` assigns a global here —
    // pre-existing compiler behaviour, tracked for the bug sweep).
    let src = "
        local M = {}
        function M.even(n) if n == 0 then return true end return M.odd(n - 1) end
        function M.odd(n) if n == 0 then return false end return M.even(n - 1) end
        return M.even(300001)";
    assert_eq!(run(src), Ok(ExVal::Bool(false)));
}

#[test]
fn tail_call_passes_all_results_through() {
    // `return f()` must forward every value f returns, not just the first.
    let src = "
        local function pair() return 3, 4 end
        local function via() return pair() end
        local a, b = via()
        return a * 10 + b";
    assert_eq!(run(src), Ok(ExVal::Integer(34)));
}

#[test]
fn tail_call_result_in_trailing_argument_position() {
    // The tail-called frame inherits MULTIRET when the caller sits in a
    // trailing-argument position.
    let src = "
        local function pair() return 5, 6 end
        local function via() return pair() end
        local function sum(a, b, c) return a + b + c end
        return sum(1, via())";
    assert_eq!(run(src), Ok(ExVal::Integer(12)));
}

#[test]
fn closures_in_tail_call_args_keep_the_callers_locals() {
    // The caller's frame is torn down before the callee runs; closures passed
    // as arguments must see its locals via closed upvalues.
    let src = "
        local function apply(f) return f() end
        local function make(x)
            local y = x * 2
            return apply(function() return x + y end)
        end
        return make(7)";
    assert_eq!(run(src), Ok(ExVal::Integer(21)));
}

#[test]
fn tail_call_with_varargs_spread() {
    let src = "
        local function sum3(a, b, c) return a + b + c end
        local function fwd(...) return sum3(...) end
        return fwd(1, 2, 3)";
    assert_eq!(run(src), Ok(ExVal::Integer(6)));
}

#[test]
fn tail_call_into_a_variadic_callee() {
    // (no `select` in the stdlib yet; count by hand. `n` is declared before
    // the `local a, b, c = ...` because a local declared *after* a multi-target
    // vararg assignment reads nil — pre-existing compiler bug, bug sweep.)
    let src = "
        local function count(...)
            local n = 0
            local a, b, c = ...
            if a ~= nil then n = n + 1 end
            if b ~= nil then n = n + 1 end
            if c ~= nil then n = n + 1 end
            return n
        end
        local function via(a) return count(a, a, a) end
        return via(9)";
    assert_eq!(run(src), Ok(ExVal::Integer(3)));
}

#[test]
fn tail_call_fills_missing_params_with_nil() {
    let src = "
        local function two(a, b) if b == nil then return a end return a + b end
        local function via() return two(5) end
        return via()";
    assert_eq!(run(src), Ok(ExVal::Integer(5)));
}

#[test]
fn tail_call_to_a_native_function() {
    let src = "
        local function via(s) return tostring(s) end
        return via(42)";
    assert_eq!(run(src), Ok(ExVal::String("42".to_string())));
}

#[test]
fn method_tail_call() {
    let src = "
        local obj = { n = 4 }
        function obj:get(k) return self.n * k end
        local function via(o) return o:get(3) end
        return via(obj)";
    assert_eq!(run(src), Ok(ExVal::Integer(12)));
}

#[test]
fn non_tail_returns_are_untouched() {
    let src = "
        local function f() return 1, 2 end
        local function a() return f() + 10 end      -- expression, not a tail call
        local function b() return f(), 7 end        -- two expressions
        local function c() local r = f() return r end
        local x, y = b()
        return a() + x + y + c()";
    assert_eq!(run(src), Ok(ExVal::Integer(11 + 1 + 7 + 1)));
}

#[test]
fn top_level_return_of_a_call_is_not_a_tail_call_and_still_works() {
    assert_eq!(
        run("local function f() return 8 end return f()"),
        Ok(ExVal::Integer(8))
    );
}
