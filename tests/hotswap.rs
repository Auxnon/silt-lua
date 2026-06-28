/// Integration tests for the hotswap feature.
///
/// The hotswap method detects what changed between two Lua source strings and
/// updates the VM with minimal disruption:
///   - No change         → NoChange
///   - Root-level diff   → RootChanged (full recompile + execute)
///   - Function-only diff → FunctionChanged (update function tree, no re-execute)

use silt_lua::{Compiler, HotswapResult, Lua};

// ── helpers ────────────────────────────────────────────────────────────────

fn make_vm() -> (Lua, Compiler) {
    (Lua::new_with_standard(), Compiler::new())
}

/// Convenience wrapper: run lua and convert errors to strings for clean panics.
fn run(lua: &mut Lua, src: &str) -> silt_lua::ExVal {
    lua.run(None, src, &mut Compiler::new())
        .map_err(|e| e.to_string())
        .unwrap()
}

fn hotswap(
    lua: &mut Lua,
    compiler: &mut Compiler,
    old: &str,
    new: &str,
) -> HotswapResult {
    lua.hotswap(None, old, new, compiler)
        .map_err(|e| e.to_string())
        .unwrap()
}

/// Returns true when `result` is `FunctionChanged` and any of the listed names match.
fn fn_changed_has(result: &HotswapResult, name: &str) -> bool {
    matches!(result, HotswapResult::FunctionChanged(v) if v.iter().any(|n| n == name))
}

// ── NoChange ───────────────────────────────────────────────────────────────

#[test]
fn hotswap_no_change() {
    let (mut lua, mut compiler) = make_vm();
    let src = "x = 1";
    lua.run(None, src, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    let result = hotswap(&mut lua, &mut compiler, src, src);
    assert_eq!(result, HotswapResult::NoChange);
}

// ── RootChanged ────────────────────────────────────────────────────────────

#[test]
fn hotswap_root_change_reexecutes() {
    let (mut lua, mut compiler) = make_vm();
    let old = "x = 10";
    let new = "x = 42";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(result, HotswapResult::RootChanged);

    // New value should be visible after root re-execution.
    let val = run(&mut lua, "return x");
    assert_eq!(val, silt_lua::ExVal::Integer(42));
}

#[test]
fn hotswap_root_change_detects_new_lines() {
    let (mut lua, mut compiler) = make_vm();
    let old = "a = 1\nb = 2";
    // Extra global added at root level.
    let new = "a = 1\nb = 2\nc = 99";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(result, HotswapResult::RootChanged);

    let val = run(&mut lua, "return c");
    assert_eq!(val, silt_lua::ExVal::Integer(99));
}

// ── FunctionChanged ────────────────────────────────────────────────────────

#[test]
fn hotswap_function_change_detected() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function greet()\n    return \"hello\"\nend";
    let new = "function greet()\n    return \"world\"\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        fn_changed_has(&result, "greet"),
        "expected FunctionChanged containing \"greet\", got {:?}",
        result
    );
}

#[test]
fn hotswap_function_change_takes_effect_after_cycle() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function add(a, b)\n    return a + b\nend";
    let new = "function add(a, b)\n    return a * b\nend";

    // Compile and run original source so `add` is defined in globals.
    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    // Hotswap: only the function body changed.
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        fn_changed_has(&result, "add"),
        "expected FunctionChanged containing \"add\", got {:?}",
        result
    );

    // Re-run (cycle) to register the new function definition into globals.
    lua.cycle()
        .map_err(|e| e.to_string())
        .unwrap();

    // Now `add` should use the new (multiply) implementation.
    let val = run(&mut lua, "return add(3, 4)");
    assert_eq!(val, silt_lua::ExVal::Integer(12)); // 3 * 4 = 12
}

#[test]
fn hotswap_function_change_preserves_other_globals() {
    let (mut lua, mut compiler) = make_vm();
    let old = "state = 100\nfunction update()\n    return state\nend";
    let new = "state = 100\nfunction update()\n    return state + 1\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    // Only the function body changed.
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        fn_changed_has(&result, "update"),
        "expected FunctionChanged containing \"update\", got {:?}",
        result
    );

    // `state` global should still be 100; root was NOT re-executed.
    let state_val = run(&mut lua, "return state");
    assert_eq!(state_val, silt_lua::ExVal::Integer(100));
}

// ── Edge-cases ─────────────────────────────────────────────────────────────

#[test]
fn hotswap_whitespace_only_change_in_function() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function foo()\n    return 1\nend";
    // Indentation changed – body line differs.
    let new = "function foo()\n  return 1\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    let result = hotswap(&mut lua, &mut compiler, old, new);
    // Body line changed → not NoChange.
    assert_ne!(result, HotswapResult::NoChange);
}

#[test]
fn hotswap_function_signature_change_treated_as_function_scope() {
    let (mut lua, mut compiler) = make_vm();
    // Changing only the function signature line is still inside the function span.
    let old = "function foo(x)\n    return x\nend";
    let new = "function foo(x, y)\n    return x + y\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    let result = hotswap(&mut lua, &mut compiler, old, new);
    // The `function` keyword line is part of the span → FunctionChanged.
    assert!(
        matches!(result, HotswapResult::FunctionChanged(_)),
        "expected FunctionChanged, got {:?}",
        result
    );
}

// ── Multiple-function hotswap ──────────────────────────────────────────────

#[test]
fn hotswap_multiple_functions_changed() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function foo()\n    return 1\nend\nfunction bar()\n    return 2\nend";
    // Both functions changed.
    let new = "function foo()\n    return 10\nend\nfunction bar()\n    return 20\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        matches!(result, HotswapResult::FunctionChanged(_)),
        "expected FunctionChanged, got {:?}",
        result
    );

    lua.cycle().map_err(|e| e.to_string()).unwrap();

    assert_eq!(run(&mut lua, "return foo()"), silt_lua::ExVal::Integer(10));
    assert_eq!(run(&mut lua, "return bar()"), silt_lua::ExVal::Integer(20));
}

#[test]
fn hotswap_unchanged_function_preserved_when_other_changes() {
    let (mut lua, mut compiler) = make_vm();
    let old =
        "function stable()\n    return 42\nend\nfunction changed()\n    return 1\nend";
    let new =
        "function stable()\n    return 42\nend\nfunction changed()\n    return 99\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        fn_changed_has(&result, "changed"),
        "expected FunctionChanged containing \"changed\", got {:?}",
        result
    );

    lua.cycle().map_err(|e| e.to_string()).unwrap();

    // `stable` should be unchanged.
    assert_eq!(run(&mut lua, "return stable()"), silt_lua::ExVal::Integer(42));
    // `changed` should now return 99.
    assert_eq!(run(&mut lua, "return changed()"), silt_lua::ExVal::Integer(99));
}

// ── Nested-function hotswap ────────────────────────────────────────────────

#[test]
fn hotswap_nested_function_change() {
    let (mut lua, mut compiler) = make_vm();
    // `outer` contains a nested function `inner`.  We change `inner`.
    let old = "function outer()\n    local function inner()\n        return 1\n    end\n    return inner()\nend";
    let new = "function outer()\n    local function inner()\n        return 99\n    end\n    return inner()\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    // The diff is inside `outer` → FunctionChanged containing "outer".
    assert!(
        fn_changed_has(&result, "outer"),
        "expected FunctionChanged containing \"outer\", got {:?}",
        result
    );

    lua.cycle().map_err(|e| e.to_string()).unwrap();

    assert_eq!(run(&mut lua, "return outer()"), silt_lua::ExVal::Integer(99));
}

// ── Anonymous-function hotswap ─────────────────────────────────────────────

#[test]
fn hotswap_anonymous_function_as_global_detected() {
    let (mut lua, mut compiler) = make_vm();
    let old = "myfn = function()\n    return 1\nend";
    let new = "myfn = function()\n    return 55\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    // The diff is inside an anonymous function → detected as FunctionChanged.
    // Root-level assignment line is line 1 (same as the `function` keyword),
    // so this may come out as RootChanged depending on line attribution.
    // Either way it must not be NoChange.
    assert_ne!(
        result,
        HotswapResult::NoChange,
        "expected a change to be detected"
    );
}

// ── Shared-line / unformatted source (root code co-located with a function) ──

#[test]
fn hotswap_shared_line_root_change_is_full_reset() {
    let (mut lua, mut compiler) = make_vm();
    // Unformatted: root assignment shares a line with a single-line function.
    let old = "counter = 5 function tick() return counter end";
    let new = "counter = 9 function tick() return counter end";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    // The root edit (`counter = 5` -> `9`) must NOT be masked as a function-only
    // change; it must be detected as a root change (full reset).
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(
        result,
        HotswapResult::RootChanged,
        "shared-line root edit must be RootChanged, got {:?}",
        result
    );
    // The root change is applied (full reload), so counter is now 9.
    assert_eq!(run(&mut lua, "return counter"), silt_lua::ExVal::Integer(9));
}

#[test]
fn hotswap_root_change_clears_stale_globals() {
    let (mut lua, mut compiler) = make_vm();
    // Old defines a global `obsolete`; new removes it and changes root code.
    let old = "obsolete = 1\nthreshold = 10";
    let new = "threshold = 20";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(result, HotswapResult::RootChanged);

    // Fresh-reset semantics: `obsolete` should be gone (not lingering), and the
    // standard library should still be available after the globals wipe.
    assert_eq!(run(&mut lua, "return obsolete"), silt_lua::ExVal::Nil);
    assert_eq!(run(&mut lua, "return threshold"), silt_lua::ExVal::Integer(20));
    assert_eq!(run(&mut lua, "return type(3)"), silt_lua::ExVal::String("number".to_string()));
}
