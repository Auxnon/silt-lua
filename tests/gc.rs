//! Garbage-collection tests: state must be correct across collection cycles, and
//! collection must actually reclaim unreachable memory.
//!
//! Collection runs automatically at the end of each executing VM cycle (debt-paced)
//! and fully after a hotswap; these tests also drive it explicitly via `collect_full`.

use silt_lua::{Compiler, ExVal, Lua};

fn run(lua: &mut Lua, src: &str) -> ExVal {
    lua.run(None, src, &mut Compiler::new())
        .map_err(|e| e.to_string())
        .unwrap()
}

/// Deterministically reclaim *all* currently-unreachable objects.
///
/// gc-arena is an INCREMENTAL collector, so a single `collect_full()` only runs the
/// collection cycle that is currently in progress to completion. If an object became
/// garbage after that cycle had already entered its mark phase (which the automatic
/// debt-paced collection in `run`/`call` leaves it partway through), it is "floating
/// garbage" — not reclaimed until the *next* cycle. Running two full cycles back to
/// back guarantees a clean slate: the first finishes/!restarts whatever was in flight,
/// the second performs a complete mark-and-sweep from the sleep phase. Production code
/// never needs this — it uses single incremental calls and lets pacing catch up over
/// time — but a test asserting exact reclamation must force determinism.
fn settle_gc(lua: &mut Lua) {
    lua.collect_full();
    lua.collect_full();
}

// ── Live state survives collection ───────────────────────────────────────────

#[test]
fn live_state_survives_full_collection() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(
        None,
        "g = 42\nt = {10, 20, 30}\nfunction dbl(x) return x * 2 end",
        &mut c,
    )
    .map_err(|e| e.to_string())
    .unwrap();

    // Force full collection. Everything reachable from globals must survive untouched.
    settle_gc(&mut lua);

    assert_eq!(run(&mut lua, "return g"), ExVal::Integer(42));
    assert_eq!(run(&mut lua, "return t[2]"), ExVal::Integer(20));
    assert_eq!(run(&mut lua, "return dbl(21)"), ExVal::Integer(42));
}

#[test]
fn closure_upvalues_survive_collection() {
    // A counter closure keeps its captured upvalue alive across collections.
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(
        None,
        "function counter()\n    local n = 0\n    return function() n = n + 1 return n end\nend\nc = counter()",
        &mut c,
    )
    .map_err(|e| e.to_string())
    .unwrap();

    assert_eq!(run(&mut lua, "return c()"), ExVal::Integer(1));
    assert_eq!(run(&mut lua, "return c()"), ExVal::Integer(2));
    settle_gc(&mut lua);
    // The captured `n` (=2) survived; the closure keeps counting from there.
    assert_eq!(run(&mut lua, "return c()"), ExVal::Integer(3));
}

// ── Collection reclaims garbage ───────────────────────────────────────────────

#[test]
fn repeated_garbage_stays_bounded() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    // `churn` allocates many short-lived tables that are unreachable once it returns.
    lua.run(
        None,
        "keep = 7\nfunction churn()\n    local s = 0\n    for i = 1, 2000 do\n        local t = {i, i, i}\n        s = s + t[1]\n    end\n    return s\nend",
        &mut c,
    )
    .map_err(|e| e.to_string())
    .unwrap();

    // Warm once, then settle memory to a baseline.
    run(&mut lua, "return churn()");
    settle_gc(&mut lua);
    let baseline = lua.allocated_bytes();

    // Many rounds of garbage. If it were never reclaimed, ~80k tables would pile up
    // (multiple MB); with collection, memory stays near the baseline.
    for _ in 0..40 {
        run(&mut lua, "return churn()");
    }
    settle_gc(&mut lua);
    let after = lua.allocated_bytes();

    assert!(
        after < baseline + 1_000_000,
        "garbage not reclaimed: baseline {} bytes -> {} bytes after 40 churn rounds",
        baseline,
        after
    );
    // The genuinely-live global is untouched.
    assert_eq!(run(&mut lua, "return keep"), ExVal::Integer(7));
}
