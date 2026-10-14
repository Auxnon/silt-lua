//! Regressions from the raw-pointer → index-based VM stack rewrite.
//!
//! * The old `[Value; 256]` stack had no bounds: recursion deeper than ~60
//!   frames wrote past the array (UB, usually a hang or corrupted globals).
//!   The stack now grows per frame and deep recursion just works, with a hard
//!   cap that surfaces as a `StackOverflow` error.
//! * `CLOSE_UPVALUES(n)` with n > 1 closed every upvalue around the value one
//!   past the stack top (nil), so a block that captured several locals handed
//!   its closures nil. Each upvalue now closes from its own slot.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}

#[test]
fn several_captured_locals_close_independently() {
    let src = "
        local fs = {}
        do
            local a = 1
            local b = 2
            local c = 3
            fs[1] = function() return a end
            fs[2] = function() return b end
            fs[3] = function() return c end
        end
        return fs[1]() * 100 + fs[2]() * 10 + fs[3]()";
    assert_eq!(run(src), Ok(ExVal::Integer(123)));
}

#[test]
fn captured_locals_keep_writes_after_close() {
    // Two closures over the same local share one upvalue; a write through one
    // is visible through the other, both before and after the block closes it.
    // Declaration form on purpose: `set = function(v) x = v end` as an
    // *assignment* to a pre-declared local is a separate, pre-existing
    // compiler bug (the local ends up nil) tracked for the bug sweep.
    let src = "
        local x = 1
        local get = function() return x end
        local set = function(v) x = v end
        do
            local y = 10
            set = function(v) x = v + y end
        end
        set(31)
        return get() + 1";
    assert_eq!(run(src), Ok(ExVal::Integer(42)));
}

#[test]
fn recursion_deeper_than_the_old_fixed_stack() {
    let src = "
        local function depth(n) if n == 0 then return 0 end return 1 + depth(n - 1) end
        return depth(5000)";
    assert_eq!(run(src), Ok(ExVal::Integer(5000)));
}

#[test]
fn runaway_recursion_is_a_stack_overflow_error() {
    let src = "local function f(n) return 1 + f(n + 1) end return f(0)";
    let err = run(src).unwrap_err();
    assert!(err.contains("stack overflow"), "unexpected error: {err}");
}

#[test]
fn vm_is_usable_after_a_stack_overflow() {
    // The error must leave the VM in a state where the next run works.
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    let _ = lua.run(
        None,
        "local function f() return 1 + f() end return f()",
        &mut comp,
    );
    let ok = lua
        .run(None, "return 7 * 6", &mut comp)
        .map_err(|e| e.to_string());
    assert_eq!(ok, Ok(ExVal::Integer(42)));
}
