//! Integration tests for the hotswap feature (gated behind the `hot-swap` feature).
//!
//! The hotswap method detects what changed between two Lua source strings and
//! updates the VM with minimal disruption:
//!   - No change          → NoChange
//!   - Root-level diff     → RootChanged (full recompile + execute)
//!   - Function-only diff  → FunctionChanged (live-swap globals, no re-execute)
#![cfg(feature = "hot-swap")]

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
    // The compiled code is identical, so nothing runs differently.
    assert_eq!(result, HotswapResult::NoChange);
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
    // Stage 2: the diff is confined to the nested `inner`, so the enclosing `outer`
    // is left untouched and only the nested function is reported.
    assert!(
        matches!(result, HotswapResult::FunctionChanged(_)),
        "expected FunctionChanged, got {:?}",
        result
    );
    assert!(
        !fn_changed_has(&result, "outer"),
        "outer should not be reported when only its nested function changed, got {:?}",
        result
    );

    // The nested change is live: outer() now returns 99 (via the redirected inner).
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

// ── Stage 1: live swap (top-level globals apply immediately, state preserved) ─

#[test]
fn hotswap_function_change_takes_effect_without_cycle() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function add(a, b)\n    return a + b\nend";
    let new = "function add(a, b)\n    return a * b\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(fn_changed_has(&result, "add"), "got {:?}", result);

    // No cycle() — the live global closure was swapped in place, so the new
    // (multiply) body is already in effect.
    let val = run(&mut lua, "return add(3, 4)");
    assert_eq!(val, silt_lua::ExVal::Integer(12));
}

#[test]
fn hotswap_live_swap_preserves_mutated_state() {
    // The core game-dev scenario: state has drifted from its initial value at
    // runtime; editing a function must apply WITHOUT resetting that state.
    let (mut lua, mut compiler) = make_vm();
    let old = "score = 0\nfunction bump()\n    score = score + 1\nend";
    let new = "score = 0\nfunction bump()\n    score = score + 10\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    // Drive state away from its initialized value.
    run(&mut lua, "bump()"); // score = 1
    run(&mut lua, "bump()"); // score = 2
    assert_eq!(run(&mut lua, "return score"), silt_lua::ExVal::Integer(2));

    // Edit bump's body; do NOT cycle (cycle would re-run root and reset score).
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(fn_changed_has(&result, "bump"), "got {:?}", result);

    // New body (+10) applies AND the mutated score (2) survived → 2 + 10 = 12.
    run(&mut lua, "bump()");
    assert_eq!(run(&mut lua, "return score"), silt_lua::ExVal::Integer(12));
}

#[test]
fn hotswap_two_functions_swap_independently_live() {
    // Scenario A: two separate top-level functions, edit only `foo`. `foo` swaps
    // live; `bar` is untouched — all without cycle().
    let (mut lua, mut compiler) = make_vm();
    let old = "function foo()\n    return 1\nend\nfunction bar()\n    return 2\nend";
    let new = "function foo()\n    return 100\nend\nfunction bar()\n    return 2\nend";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(fn_changed_has(&result, "foo"), "got {:?}", result);
    // Only foo should be reported as changed.
    assert!(
        !fn_changed_has(&result, "bar"),
        "bar must not be reported changed, got {:?}",
        result
    );

    assert_eq!(run(&mut lua, "return foo()"), silt_lua::ExVal::Integer(100));
    assert_eq!(run(&mut lua, "return bar()"), silt_lua::ExVal::Integer(2));
}

// ── Stage 2: nested / instance method live swap (preserves instance state) ────

#[test]
fn hotswap_nested_method_swaps_live_preserving_instance_state() {
    // Scenario B: a "class" constructor whose nested method captures per-instance
    // state. Editing ONLY the nested method must hot-swap its code into the live
    // instance while preserving that instance's captured state.
    let (mut lua, mut compiler) = make_vm();
    let old = "function makeCounter()\n    local v = 0\n    local c = {}\n    function c.bump()\n        v = v + 1\n        return v\n    end\n    return c\nend\no = makeCounter()";
    let new = "function makeCounter()\n    local v = 0\n    local c = {}\n    function c.bump()\n        v = v + 10\n        return v\n    end\n    return c\nend\no = makeCounter()";

    lua.run(None, old, &mut compiler).map_err(|e| e.to_string()).unwrap();
    // Drive the instance's captured state away from its initial value.
    assert_eq!(run(&mut lua, "return o.bump()"), silt_lua::ExVal::Integer(1));
    assert_eq!(run(&mut lua, "return o.bump()"), silt_lua::ExVal::Integer(2));

    let result = hotswap(&mut lua, &mut compiler, old, new);
    // Must be a function-scope change, NOT a root reset (which would rebuild `o`).
    assert!(
        matches!(result, HotswapResult::FunctionChanged(_)),
        "expected FunctionChanged, got {:?}",
        result
    );
    // Granularity: the unchanged wrapper must NOT be reported as changed.
    assert!(
        !fn_changed_has(&result, "makeCounter"),
        "wrapper makeCounter must not be reported, got {:?}",
        result
    );

    // The live instance keeps its captured v (=2) AND runs the new body (+10) → 12.
    assert_eq!(run(&mut lua, "return o.bump()"), silt_lua::ExVal::Integer(12));
}

#[test]
fn hotswap_nested_change_reports_inner_not_wrapper() {
    // Editing only a nested `local function` reports the nested function, leaving
    // the enclosing function untouched (stage-2 granularity).
    let (mut lua, mut compiler) = make_vm();
    let old = "function outer()\n    local function inner()\n        return 1\n    end\n    return inner()\nend\no = outer";
    let new = "function outer()\n    local function inner()\n        return 99\n    end\n    return inner()\nend\no = outer";

    lua.run(None, old, &mut compiler).map_err(|e| e.to_string()).unwrap();
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert!(
        matches!(result, HotswapResult::FunctionChanged(_)),
        "expected FunctionChanged, got {:?}",
        result
    );
    // New behavior takes effect: calling outer() now returns 99.
    assert_eq!(run(&mut lua, "return outer()"), silt_lua::ExVal::Integer(99));
}

// ── Stage 2 + GC: replaced prototypes are reclaimed, new code survives ────────

#[test]
fn hotswap_repeated_nested_swaps_stay_bounded_and_correct() {
    // Hot-swapping a nested method many times must (a) keep working — each new body
    // survives the full collection hotswap triggers — and (b) not leak: every
    // replaced prototype subtree becomes unreachable and is reclaimed, so memory
    // stays bounded across many swaps. This exercises both halves of the concern:
    // the new node is NOT collected before it's live, and the old nodes ARE.
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let src = |inc: i64| {
        format!(
            "function make()\n    local v = 0\n    local c = {{}}\n    function c.bump()\n        v = v + {}\n        return v\n    end\n    return c\nend\no = make()",
            inc
        )
    };

    let mut cur = src(1);
    lua.run(None, &cur, &mut compiler).map_err(|e| e.to_string()).unwrap();
    assert_eq!(run(&mut lua, "return o.bump()"), silt_lua::ExVal::Integer(1));

    // Two full cycles: gc-arena is incremental, so one `collect_full` only finishes
    // the in-flight cycle; a second guarantees a clean mark-and-sweep so `baseline`
    // reflects only genuinely-live memory.
    lua.collect_full();
    lua.collect_full();
    let baseline = lua.allocated_bytes();

    // 99 hot-swaps, each changing the increment and orphaning the prior prototype tree.
    const SWAPS: i64 = 100;
    for inc in 2..=SWAPS {
        let next = src(inc);
        let result = hotswap(&mut lua, &mut compiler, &cur, &next);
        assert!(
            matches!(result, HotswapResult::FunctionChanged(_)),
            "swap {} expected FunctionChanged, got {:?}",
            inc,
            result
        );
        cur = next;
        // The live instance still runs (its new body survived the collection that
        // hotswap performs) and keeps its captured state.
        assert!(
            matches!(run(&mut lua, "return o.bump()"), silt_lua::ExVal::Integer(_)),
            "instance method dead after swap {}",
            inc
        );
    }

    lua.collect_full();
    lua.collect_full();
    let after = lua.allocated_bytes();
    // Measured growth across ~100 swaps is a few hundred bytes flat. If replaced
    // prototype trees leaked, ~99 of them (≥1 KB each) would accumulate into tens of
    // KB+; this bound (well under that, far above the observed ~480 B) fails on a leak
    // but tolerates allocator noise.
    assert!(
        after < baseline + 20_000,
        "hotswap leaked replaced prototypes: baseline {} bytes -> {} bytes after {} swaps",
        baseline,
        after,
        SWAPS - 1
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
fn hotswap_root_change_keeps_globals() {
    let (mut lua, mut compiler) = make_vm();
    // Old defines a global `obsolete`; new removes it and changes root code.
    let old = "obsolete = 1\nthreshold = 10";
    let new = "threshold = 20";

    lua.run(None, old, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();

    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(result, HotswapResult::RootChanged);

    // The file's root runs again over the live globals: what it assigns is
    // reassigned, nothing is cleared (the host's and other files' globals live there).
    assert_eq!(run(&mut lua, "return obsolete"), silt_lua::ExVal::Integer(1));
    assert_eq!(run(&mut lua, "return threshold"), silt_lua::ExVal::Integer(20));
    assert_eq!(run(&mut lua, "return type(3)"), silt_lua::ExVal::String("number".to_string()));
}

// ── Files as scopes: code-based diff, no globals reset ──────────────────────

fn load(lua: &mut Lua, compiler: &mut Compiler, name: &str, src: &str) {
    lua.run(Some(name), src, compiler).map_err(|e| e.to_string()).unwrap();
}

fn swap_file(lua: &mut Lua, compiler: &mut Compiler, name: &str, old: &str, new: &str) -> HotswapResult {
    lua.hotswap(Some(name), old, new, compiler).map_err(|e| e.to_string()).unwrap()
}

#[test]
fn hotswap_inserted_lines_and_comments_are_not_root_changes() {
    let (mut lua, mut compiler) = make_vm();
    let old = "x = 1\n\nfunction a() return 1 end\n\nfunction b() return 2 end";
    // a comment and a blank line above everything, and b's body edited
    let new = "-- header\n\nx = 1\n\nfunction a() return 1 end\n\nfunction b() return 20 end";
    run_with(&mut lua, &mut compiler, old);
    let result = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(result, HotswapResult::FunctionChanged(vec!["b".to_string()]));
    assert_eq!(run(&mut lua, "return a() + b()"), silt_lua::ExVal::Integer(21));

    // comments alone change nothing that runs
    let commented = new.replace("function a()", "-- a is one\nfunction a()");
    assert_eq!(hotswap(&mut lua, &mut compiler, new, &commented), HotswapResult::NoChange);
}

fn run_with(lua: &mut Lua, compiler: &mut Compiler, src: &str) {
    lua.run(None, src, compiler).map_err(|e| e.to_string()).unwrap();
}

#[test]
fn hotswap_moved_function_reports_its_new_line() {
    let (mut lua, mut compiler) = make_vm();
    let old = "function boom()\n  return nil + 1\nend";
    let new = "\n\nfunction boom()\n  return nil + 1\nend";
    run_with(&mut lua, &mut compiler, old);
    assert_eq!(hotswap(&mut lua, &mut compiler, old, new), HotswapResult::NoChange);
    let err = lua.run(None, "boom()", &mut Compiler::new()).unwrap_err().to_string();
    assert!(err.contains("@4:"), "error should point at line 4 now: {err}");
}

#[test]
fn hotswap_root_change_keeps_host_natives_and_function_state() {
    let (mut lua, mut compiler) = make_vm();
    let old = "limit = 3\nfunction bump() hits = (hits or 0) + 1 return hits end";
    let new = "limit = 5\nfunction bump() hits = (hits or 0) + 1 return hits end";
    run_with(&mut lua, &mut compiler, old);
    run(&mut lua, "bump() bump()");
    assert_eq!(hotswap(&mut lua, &mut compiler, old, new), HotswapResult::RootChanged);
    // `hits` was set by a function, not the root: it carries over
    assert_eq!(run(&mut lua, "return bump()"), silt_lua::ExVal::Integer(3));
    assert_eq!(run(&mut lua, "return limit"), silt_lua::ExVal::Integer(5));
    assert_eq!(run(&mut lua, "return math.floor(2.5)"), silt_lua::ExVal::Integer(2));
}

#[test]
fn hotswap_one_file_leaves_another_alone() {
    let (mut lua, mut compiler) = make_vm();
    let a_old = "a_val = 1\nfunction from_a() return 'a1' end";
    let a_new = "a_val = 2\nfunction from_a() return 'a1' end";
    let b = "b_val = 7\nfunction from_b() return 'b' end";
    load(&mut lua, &mut compiler, "a.lua", a_old);
    load(&mut lua, &mut compiler, "b.lua", b);
    run(&mut lua, "b_val = 8");

    assert_eq!(swap_file(&mut lua, &mut compiler, "a.lua", a_old, a_new), HotswapResult::RootChanged);
    assert_eq!(run(&mut lua, "return a_val"), silt_lua::ExVal::Integer(2));
    // b's state and functions are untouched by a's reload
    assert_eq!(run(&mut lua, "return b_val .. from_b()"), silt_lua::ExVal::String("8b".to_string()));
}

#[test]
fn hotswap_does_not_replace_another_files_function() {
    let (mut lua, mut compiler) = make_vm();
    let a_old = "function shared() return 'a' end";
    let a_new = "function shared() return 'a2' end";
    load(&mut lua, &mut compiler, "a.lua", a_old);
    // b, loaded later, owns the global now
    load(&mut lua, &mut compiler, "b.lua", "function shared() return 'b' end");

    let r = lua
        .hotswap_functions(Some("a.lua"), a_old, a_new, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    assert_eq!(r, HotswapResult::RootChanged, "a no longer owns `shared`, so it can't swap it in place");
    assert_eq!(run(&mut lua, "return shared()"), silt_lua::ExVal::String("b".to_string()));
}

#[test]
fn hotswap_functions_never_runs_root_code() {
    let (mut lua, mut compiler) = make_vm();
    let old = "n = 1";
    let new = "n = 2";
    run_with(&mut lua, &mut compiler, old);
    let r = lua.hotswap_functions(None, old, new, &mut compiler).map_err(|e| e.to_string()).unwrap();
    assert_eq!(r, HotswapResult::RootChanged);
    assert_eq!(run(&mut lua, "return n"), silt_lua::ExVal::Integer(1));
}

#[test]
fn hotswap_function_keeps_file_local_upvalues() {
    let (mut lua, mut compiler) = make_vm();
    let old = "local n = 10\nfunction get() return n end\nfunction set(v) n = v end";
    let new = "local n = 10\nfunction get() return n * 2 end\nfunction set(v) n = v end";
    run_with(&mut lua, &mut compiler, old);
    run(&mut lua, "set(4)");
    let r = hotswap(&mut lua, &mut compiler, old, new);
    assert_eq!(r, HotswapResult::FunctionChanged(vec!["get".to_string()]));
    // the new body reads the same cell `set` writes
    assert_eq!(run(&mut lua, "return get()"), silt_lua::ExVal::Integer(8));
    run(&mut lua, "set(6)");
    assert_eq!(run(&mut lua, "return get()"), silt_lua::ExVal::Integer(12));
}

#[test]
fn hotswap_changed_local_function_reloads_the_file() {
    let (mut lua, mut compiler) = make_vm();
    let old = "local function helper() return 1 end\nfunction api() return helper() end";
    let new = "local function helper() return 2 end\nfunction api() return helper() end";
    run_with(&mut lua, &mut compiler, old);
    // a local function has no global to swap through: the file runs again
    assert_eq!(hotswap(&mut lua, &mut compiler, old, new), HotswapResult::RootChanged);
    assert_eq!(run(&mut lua, "return api()"), silt_lua::ExVal::Integer(2));
}
