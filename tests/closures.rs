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

#[test]
fn method_definition_colon_with_args() {
    // Implicit `self` must occupy slot 0 and the explicit param `n` slot 1.
    valeq!(
        r#"
        local t = { x = 10 }
        function t:add(n) return self.x + n end
        return t:add(5)
    "#,
        ExVal::Integer(15)
    );
}

#[test]
fn dot_function_definition() {
    // `function t.get(self)` — field assignment, no implicit self.
    valeq!(
        r#"
        local t = { x = 9 }
        function t.get(s) return s.x end
        return t.get(t)
    "#,
        ExVal::Integer(9)
    );
}

#[test]
fn method_call_on_chained_receiver() {
    // `a.b:m(...)` — the receiver is a chained field access, not a bare
    // variable. METHOD_GET must use the fully-resolved receiver as `self`.
    valeq!(
        r#"
        local a = { b = { x = 100 } }
        function a.b:add(n) return self.x + n end
        return a.b:add(5)
    "#,
        ExVal::Integer(105)
    );
}

#[test]
fn method_call_deep_chain() {
    // depth-3 receiver chain feeding a colon call
    valeq!(
        r#"
        local a = { b = { c = { x = 7 } } }
        function a.b.c:get() return self.x end
        return a.b.c:get()
    "#,
        ExVal::Integer(7)
    );
}

// Deep nesting: a function three or more levels down capturing a *grandparent's* local. The
// compiler's per-function local offset was computed from a cumulative sum of earlier offsets,
// double-counting them, so the inner function's own params landed one slot too low (slot 0 =
// the function itself): `base + x` became "Cannot + 'integer' and '(function)'".

#[test]
fn three_level_capture_of_root_local() {
    valeq!(
        r#"
        local base = 100
        local function outer(k)
            local function mk(m) return function(x) return base + x end end
            return mk(k)(2)
        end
        return outer(5)
    "#,
        ExVal::Integer(102)
    );
}

#[test]
fn three_level_capture_mixes_parent_and_grandparent() {
    valeq!(
        r#"
        local base = 100
        local function outer(k)
            local function mk(m) return function(x) return base + m * x end end
            return mk(k)
        end
        return outer(5)(2)
    "#,
        ExVal::Integer(110)
    );
}

#[test]
fn four_level_capture_chain() {
    valeq!(
        r#"
        local a = 1000
        local function f1()
            local b = 200
            return function()
                local c = 30
                return function()
                    local d = 4
                    return function() return a + b + c + d end
                end
            end
        end
        return f1()()()()
    "#,
        ExVal::Integer(1234)
    );
}

#[test]
fn root_local_captured_by_a_global_survives_the_next_chunk() {
    // A chunk's locals are closed when the chunk ends. The upvalue used to stay open on the
    // dead stack slot, so the next chunk's first local silently took its place (`get()`
    // returned 99 here).
    let mut lua = silt_lua::Lua::new_with_standard();
    let mut c = silt_lua::Compiler::new();
    lua.run(None, "local n = 10 function get() return n end", &mut c)
        .map_err(|e| e.to_string())
        .unwrap();
    assert_eq!(
        lua.run(None, "local m = 99 return get()", &mut c)
            .map_err(|e| e.to_string())
            .unwrap(),
        ExVal::Integer(10)
    );
}

#[test]
fn root_local_counter_keeps_counting_across_chunks() {
    let mut lua = silt_lua::Lua::new_with_standard();
    let mut c = silt_lua::Compiler::new();
    lua.run(
        None,
        "local n = 0 function inc() n = n + 1 return n end",
        &mut c,
    )
    .map_err(|e| e.to_string())
    .unwrap();
    assert_eq!(
        lua.run(None, "local x, y = 100, 200 return inc()", &mut c)
            .map_err(|e| e.to_string())
            .unwrap(),
        ExVal::Integer(1)
    );
    assert_eq!(
        lua.run(None, "return inc()", &mut c)
            .map_err(|e| e.to_string())
            .unwrap(),
        ExVal::Integer(2)
    );
}
