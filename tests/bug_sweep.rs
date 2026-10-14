//! Regressions from the 2026-10 bug sweep. Each test names the bug it pins.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}
fn int(src: &str, want: i64) {
    assert_eq!(run(src), Ok(ExVal::Integer(want)), "{src}");
}

// --- A reused Compiler carried per-compile state (locals, scope depth, var
// stacks, `valid`) into the next chunk, so the second run's locals read the
// first run's slots. ---------------------------------------------------------

#[test]
fn reusing_a_compiler_across_runs_keeps_locals_correct() {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    let r = |lua: &mut Lua, comp: &mut Compiler, src: &str| {
        lua.run(None, src, comp).map_err(|e| e.to_string())
    };
    assert_eq!(
        r(&mut lua, &mut comp, "local t = {} return 0"),
        Ok(ExVal::Integer(0))
    );
    assert_eq!(
        r(&mut lua, &mut comp, "local t = {} t[1] = 5 return t[1]"),
        Ok(ExVal::Integer(5))
    );
    assert_eq!(
        r(
            &mut lua,
            &mut comp,
            "local x = 1 local t = {} for i = 1, 3 do t[i] = i end return t[3] + x"
        ),
        Ok(ExVal::Integer(4))
    );
}

#[test]
fn a_failed_compile_does_not_poison_the_next_one() {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    assert!(lua.run(None, "local = 5", &mut comp).is_err());
    assert_eq!(
        lua.run(None, "return 6 * 7", &mut comp)
            .map_err(|e| e.to_string()),
        Ok(ExVal::Integer(42))
    );
}

// --- `set = function(v) x = v end`: the body's assignment drained the outer
// statement's setter from the shared var stack, so `set` stayed nil. ---------

#[test]
fn assigning_a_closure_that_writes_an_upvalue_to_a_local() {
    int(
        "local x = 1 local set set = function(v) x = v end set(41) return x + 1",
        42,
    );
}

#[test]
fn assigning_closures_inside_a_block_to_outer_locals() {
    int(
        "local get, set
         do
            local x = 1
            get = function() return x end
            set = function(v) x = v end
         end
         set(41)
         return get() + 1",
        42,
    );
}

#[test]
fn multi_assignment_with_a_closure_on_the_right() {
    int(
        "local x = 0
         local a, b = 5, function(v) x = v end
         b(10)
         return a + x",
        15,
    );
}

// --- `local f` then `function f() end` defined a global and left the local nil.

#[test]
fn function_statement_assigns_an_existing_local() {
    int("local f function f(n) return n + 1 end return f(1)", 2);
}

#[test]
fn forward_declared_mutual_recursion() {
    let src = "
        local is_even, is_odd
        function is_even(n) if n == 0 then return true end return is_odd(n - 1) end
        function is_odd(n) if n == 0 then return false end return is_even(n - 1) end
        return is_even(10) and 1 or 0";
    int(src, 1);
}

#[test]
fn function_statement_assigns_an_enclosing_functions_local() {
    // `f` is an upvalue of the inner block that defines it.
    int(
        "local function outer()
            local f
            local function define() function f() return 7 end end
            define()
            return f()
         end
         return outer()",
        7,
    );
}

#[test]
fn function_statement_without_a_local_still_defines_a_global() {
    int("function g() return 3 end return g()", 3);
}

// --- `...` reserved a phantom local slot, so every local declared after any
// vararg use in the function read the wrong slot. ----------------------------

#[test]
fn locals_declared_after_a_vararg_assignment() {
    int("local function f(...) local a, b, c = ... local n = 0 return n + a + b + c end return f(1, 2, 3)", 6);
    int(
        "local function f(...) local a = ... local n = 5 return n end return f(1)",
        5,
    );
}

#[test]
fn locals_declared_after_a_vararg_call_argument() {
    int(
        "local function f(...) print(...) local n = 5 return n end return f(1, 2)",
        5,
    );
}

// --- `{...}` compiled a fixed-count TABLE_BUILD, so the table was never found
// at the expected stack slot ("compilation corruption"). ----------------------

#[test]
fn table_constructor_from_varargs() {
    int(
        "local function f(...) local t = {...} return #t end return f(1, 2, 3)",
        3,
    );
    int(
        "local function f(...) local t = {...} return t[2] end return f(1, 2, 3)",
        2,
    );
    int("local function f(...) return {...} end return #f(1, 2)", 2);
    int(
        "local function f(...) local t = {0, ...} return #t * 10 + t[3] end return f(7, 8)",
        38,
    );
}

#[test]
fn vararg_in_a_table_is_truncated_when_not_last() {
    int(
        "local function f(...) local t = {..., 9} return #t * 10 + t[2] end return f(1, 2, 3)",
        29,
    );
    int(
        "local function f(...) local t = {x = ...} return t.x end return f(4, 5)",
        4,
    );
}

// --- `select` was missing from the stdlib. -----------------------------------

#[test]
fn select_count_and_positive_index() {
    int(
        "local function f(...) return select('#', ...) end return f(1, 2, 3)",
        3,
    );
    int(
        "local function f(...) local a, b = select(2, ...) return a * 10 + b end return f(9, 1, 2)",
        12,
    );
    int("local function f(...) return select('#') end return f()", 0);
}

#[test]
fn select_negative_index_counts_from_the_end() {
    int(
        "local function f(...) local a = select(-1, ...) return a end return f(1, 2, 3)",
        3,
    );
    int("local function f(...) local a, b = select(-2, ...) return a * 10 + b end return f(1, 2, 3)", 23);
}
