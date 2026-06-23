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
#[ignore = "PLAN.md §2 — forwarding varargs into another call (multi-value tail position)"]
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
