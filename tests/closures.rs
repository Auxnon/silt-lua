// Closures / upvalue capture (PLAN.md §1.1 — FIXED). The upvalue resolver used to crash with
// `index out of bounds` in `resolve_upvalue` (it indexed `functional_states` by functional
// depth instead of depth-1; root has no entry in that vec, it uses `root_state`). It also
// failed to actually close upvalues on function return — `close_upvalues_by_return` mutated a
// throwaway copy of the UpValue, leaving the real heap cell open on dead stack memory.
// `method_definition_colon` below stays ignored: that is the separate §1.2 colon-method parser
// path (`todo!` in `typing`), not an upvalue bug.
use silt_lua::{simple, valeq, ExVal};

#[test]
fn counter_closure() {
    valeq!(
        r#"
        local function make_counter()
            local count = 0
            return function() count = count + 1 return count end
        end
        local c = make_counter()
        c(); c()
        return c()
    "#,
        ExVal::Integer(3)
    );
}

#[test]
fn for_loop_capture() {
    // Each iteration should capture its own `i`.
    valeq!(
        r#"
        local a = {}
        for i = 1, 3 do
            a[i] = function() return i end
        end
        return a[1]() + a[2]() + a[3]()
    "#,
        ExVal::Integer(6)
    );
}

#[test]
fn nested_capture() {
    valeq!(
        r#"
        local function f1()
            local x = 2
            local function f2() return x end
            return f2
        end
        return f1()()
    "#,
        ExVal::Integer(2)
    );
}

#[test]
fn multi_level_upvalue() {
    valeq!(
        r#"
        do
            local a = 1
            local b = 2
            local function f1()
                local c = 3
                local function f2()
                    local d = 4
                    c = c + d
                    return a + b + c + d
                end
                return f2
            end
            local x = f1()
            return x() + x()
        end
    "#,
        ExVal::Integer(32)
    );
}

#[test]
fn shared_upvalue_between_closures() {
    // Two closures capturing the SAME enclosing locals must share one upvalue
    // each, so a write through one is visible through the other. Regressed when
    // the `open_upvalues` dedup scan bailed early and created a duplicate cell;
    // the duplicate then closed over an already-nil'd slot. `g` mutates a and b,
    // and `f` must observe those mutations.
    valeq!(
        r#"
        local function mk()
            local a = 1
            local b = 2
            local f = function() return a + b end
            local g = function() a = a + 100 b = b + 1 return a + b end
            return f, g
        end
        local f, g = mk()
        g()
        return f()
    "#,
        ExVal::Integer(104)
    );
}

#[test]
#[ignore = "PLAN.md §1.2 — colon-method definition unimplemented (parser todo! in `typing`), not an upvalue bug"]
fn method_definition_colon() {
    valeq!(
        r#"
        local t = { x = 5 }
        function t:get() return self.x end
        return t:get()
    "#,
        ExVal::Integer(5)
    );
}
