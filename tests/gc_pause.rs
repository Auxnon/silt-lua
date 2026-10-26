//! A long-running script pauses for collection whenever enough allocation debt
//! builds up (`Lua::drive`), then resumes where it left off. These scripts allocate
//! far past the pause threshold, so they cross many pauses: everything still
//! reachable (stack, locals, upvalues, tables, globals) must survive each one, and
//! the garbage must actually be freed.

use silt_lua::{Compiler, ExVal, Lua};

fn run(lua: &mut Lua, src: &str) -> ExVal {
    let mut compiler = Compiler::new();
    lua.run(None, src, &mut compiler).unwrap_or_else(|e| panic!("{:?}", e))
}

#[test]
fn garbage_is_collected_while_the_script_runs() {
    let mut lua = Lua::new_with_standard();
    let src = r#"
        local n = 0
        for i = 1, 300000 do
            local s = "item" .. i
            local t = { s, i }
            n = n + #t[1]
        end
        return n
    "#;
    // sum of #("item" .. i) for i in 1..=300000
    let expect: i64 = (1..=300000i64).map(|i| 4 + i.to_string().len() as i64).sum();
    assert_eq!(run(&mut lua, src), ExVal::Integer(expect));
    // 300k strings + tables would be tens of MB if nothing was freed mid-run
    lua.collect_full();
    assert!(lua.allocated_bytes() < 4 * 1024 * 1024, "{}", lua.allocated_bytes());
}

#[test]
fn live_values_survive_pauses() {
    let mut lua = Lua::new_with_standard();
    let src = r#"
        local keep = {}
        local prefix = "k" .. "ey"           -- a runtime string held in a local
        global_table = {}
        for i = 1, 50000 do
            keep[i] = prefix .. i            -- reachable through a local table
            global_table["g" .. i] = i       -- string keys of a global table
            local junk = { "x" .. i, "y" .. i } -- garbage, forces pauses
        end
        local total = 0
        for i = 1, 50000 do
            if keep[i] ~= "key" .. i then return "lost keep " .. i end
            if global_table["g" .. i] ~= i then return "lost global " .. i end
            total = total + 1
        end
        return total
    "#;
    assert_eq!(run(&mut lua, src), ExVal::Integer(50000));
}

#[test]
fn closures_and_upvalues_survive_pauses() {
    let mut lua = Lua::new_with_standard();
    let src = r#"
        local function counter(name)
            local n = 0
            return function()
                n = n + 1
                return name .. ":" .. n
            end
        end
        local cs = {}
        for i = 1, 2000 do cs[i] = counter("c" .. i) end
        local last
        for round = 1, 20 do
            for i = 1, 2000 do last = cs[i]() end
        end
        return last
    "#;
    assert_eq!(run(&mut lua, src), ExVal::String("c2000:20".into()));
}

#[test]
fn nested_runs_and_multiple_returns_across_pauses() {
    // pcall runs its callee on a nested loop (which never pauses); the outer loop
    // pauses around it. Multi-value returns flow through allocating calls.
    let mut lua = Lua::new_with_standard();
    let src = r#"
        local function pair(i) return "a" .. i, "b" .. i end
        local count = 0
        for i = 1, 40000 do
            local ok, v = pcall(function() return ("v" .. i) .. "!" end)
            local a, b = pair(i)
            if ok and v == "v" .. i .. "!" and a == "a" .. i and b == "b" .. i then
                count = count + 1
            end
        end
        return count
    "#;
    assert_eq!(run(&mut lua, src), ExVal::Integer(40000));
}

#[test]
fn stored_function_calls_pause_too() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let idx = lua
        .load_fn(
            &mut compiler,
            Some("cb"),
            r#"
            local n = ...
            local parts = {}
            for i = 1, n do parts[i] = tostring(i) .. "," end
            return #parts[n]
            "#,
        )
        .unwrap();
    for _ in 0..3 {
        let v = lua.call_with_params(Some("cb"), idx, 100000i64).unwrap();
        assert_eq!(v, ExVal::Integer(7));
    }
}

#[test]
fn errors_after_a_pause_are_reported() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let src = r#"
        local t = {}
        for i = 1, 100000 do t[i] = "s" .. i end
        return t.missing.field
    "#;
    assert!(lua.run(None, src, &mut compiler).is_err());
    // and the VM is usable afterwards
    assert_eq!(run(&mut lua, "return 1 + 1"), ExVal::Integer(2));
}
