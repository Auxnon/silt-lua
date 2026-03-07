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
        matches!(result, HotswapResult::FunctionChanged(ref n) if n == "greet"),
        "expected FunctionChanged(\"greet\"), got {:?}",
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
        matches!(result, HotswapResult::FunctionChanged(ref n) if n == "add"),
        "expected FunctionChanged(\"add\"), got {:?}",
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
        matches!(result, HotswapResult::FunctionChanged(ref n) if n == "update"),
        "expected FunctionChanged(\"update\"), got {:?}",
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
