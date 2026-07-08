use silt_lua::{simple, valeq, ExVal};

#[test]
fn vararg_first() {
    // `local a = ...` binds the first vararg.
    valeq!(
        r#"
        function test(...)
            local a = ...
            return a
        end
        return test(1, 3, 7, 12)
        "#,
        ExVal::Integer(1)
    );
}

// =====================================================================================
// Vararg features still WIP on this branch — see PLAN.md §2 / §3.
// =====================================================================================

#[test]
#[ignore = "PLAN.md §3 — `{...}` table-from-varargs reports 'compilation corruption'"]
fn vararg_to_table() {
    valeq!(
        r#"
        local function f(...) return {...} end
        local t = f(1, 2, 3)
        return #t
        "#,
        ExVal::Integer(3)
    );
}

#[test]
#[ignore = "PLAN.md §3 — `select('#', ...)` needs `select` implemented"]
fn vararg_count() {
    valeq!(
        r#"
        local function f(...) return select('#', ...) end
        return f(1, 2, 3)
        "#,
        ExVal::Integer(3)
    );
}

#[test]
fn vararg_forward() {
    valeq!(
        r#"
        local function sum(a, b, c) return a + b + c end
        local function f(...) return sum(...) end
        return f(1, 2, 3)
        "#,
        ExVal::Integer(6)
    );
}

#[test]
fn call_returns_spread_in_call_position() {
    // A trailing function-call argument spreads ALL its return values (open multiret).
    valeq!("local function g() return 3,4 end local function add(a,b) return a+b end return add(g())", ExVal::Integer(7));
    // table.unpack spreads the same way.
    valeq!("local function add(a,b) return a+b end return add(table.unpack({3,4}))", ExVal::Integer(7));
    // Fixed args before a trailing multiret call are preserved.
    valeq!("local function g() return 2,3 end local function s(a,b,c) return a+b+c end return s(1, g())", ExVal::Integer(6));
    // A call that is NOT the last argument is truncated to a single value (Lua rule).
    valeq!("local function g() return 2,3 end local function add(a,b) return a+b end return add(g(), 100)", ExVal::Integer(102));
}
