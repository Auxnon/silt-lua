//! Table memory: what tables hold must be counted, freed with them, and given back
//! when a long-lived table empties, and the usual ways of emptying a table (`t[k] =
//! nil` inside `pairs`) must actually empty it.
//!
//! `allocated_bytes` counts everything the arena tracks, including the array and
//! hash buffers tables report (they live outside the `Gc` allocation). Sizes are
//! asserted loosely, in multiples of what the entries cost, so the tests don't pin
//! the exact layout.

use silt_lua::{Compiler, ExVal, Lua};

fn run(lua: &mut Lua, src: &str) -> ExVal {
    lua.run(None, src, &mut Compiler::new())
        .map_err(|e| e.to_string())
        .unwrap()
}

/// Two full cycles: the first finishes whatever incremental cycle was in flight,
/// the second sweeps from scratch (see `settle_gc` in tests/gc.rs).
fn settle(lua: &mut Lua) -> usize {
    lua.collect_full();
    lua.collect_full();
    lua.allocated_bytes()
}

/// One table entry's least possible cost: a 32-byte key plus a 32-byte value
/// would be more, so 16 bytes per entry is a safe floor for both parts.
const ENTRY: usize = 16;
const N: usize = 50_000;

// ── Counting ─────────────────────────────────────────────────────────────────

#[test]
fn array_storage_is_counted() {
    let mut lua = Lua::new_with_standard();
    let base = settle(&mut lua);
    run(&mut lua, &format!("big = {{}} for i = 1, {N} do big[i] = i end"));
    let after = settle(&mut lua);
    assert!(
        after > base + N * ENTRY,
        "a {N}-element array barely registered: {base} -> {after} bytes"
    );
}

#[test]
fn hash_storage_is_counted() {
    let mut lua = Lua::new_with_standard();
    let base = settle(&mut lua);
    // float keys: no string allocations to inflate the count, only the hash part
    run(&mut lua, &format!("big = {{}} for i = 1, {N} do big[i + 0.5] = i end"));
    let after = settle(&mut lua);
    assert!(
        after > base + N * ENTRY,
        "a {N}-key hash barely registered: {base} -> {after} bytes"
    );
}

#[test]
fn a_dropped_table_gives_its_storage_back() {
    let mut lua = Lua::new_with_standard();
    let base = settle(&mut lua);
    run(
        &mut lua,
        &format!("big = {{}} for i = 1, {N} do big[i] = i big[i + 0.5] = i end"),
    );
    let full = settle(&mut lua);
    run(&mut lua, "big = nil");
    let after = settle(&mut lua);
    assert!(full > base + 2 * N * ENTRY, "not counted: {base} -> {full}");
    assert!(
        after < base + 10_000,
        "dropped table kept its storage: {base} -> {full} -> {after} bytes"
    );
}

// ── Long-lived tables ────────────────────────────────────────────────────────

#[test]
fn an_emptied_array_shrinks() {
    let mut lua = Lua::new_with_standard();
    run(&mut lua, "keep = {}");
    let base = settle(&mut lua);
    run(&mut lua, &format!("for i = 1, {N} do keep[i] = i end"));
    let full = settle(&mut lua);
    // pop from the top, as a stack would
    run(&mut lua, &format!("for i = {N}, 1, -1 do keep[i] = nil end"));
    let after = settle(&mut lua);
    assert!(
        after < base + (full - base) / 4,
        "emptied array kept its buffer: {base} -> {full} -> {after} bytes"
    );
    assert_eq!(run(&mut lua, "return #keep"), ExVal::Integer(0));
}

#[test]
fn an_emptied_hash_shrinks_on_its_next_growth() {
    // Clearing keys leaves them dead (so `pairs` can keep going); the space comes
    // back when new keys need room, as in Lua.
    // smaller than N: `next` finds its place in the hash part by a linear scan,
    // so a `pairs` traversal is quadratic (a known, separate issue)
    const N: usize = 5_000;
    let mut lua = Lua::new_with_standard();
    run(&mut lua, "keep = {}");
    let base = settle(&mut lua);
    run(&mut lua, &format!("for i = 1, {N} do keep[i + 0.5] = i end"));
    let full = settle(&mut lua);
    run(&mut lua, "for k in pairs(keep) do keep[k] = nil end");
    // new keys (each cleared again) until the map is full and has to make room
    run(&mut lua, &format!("for i = 1, {N} do keep[-i - 0.5] = i keep[-i - 0.5] = nil end"));
    run(&mut lua, "keep.x = 1");
    let after = settle(&mut lua);
    assert!(
        after < base + (full - base) / 10,
        "emptied hash never shrank: {base} -> {full} -> {after} bytes"
    );
    assert_eq!(run(&mut lua, "local n = 0 for _ in pairs(keep) do n = n + 1 end return n"), ExVal::Integer(1));
}

#[test]
fn key_churn_on_a_long_lived_table_stays_bounded() {
    // A cache keyed by ever-new keys, holding at most 100 at a time: dead keys
    // must be swept, not pile up.
    let mut lua = Lua::new_with_standard();
    run(
        &mut lua,
        "cache = {} serial = 0
         function churn(n)
             for _ = 1, n do
                 serial = serial + 1
                 cache[serial + 0.5] = serial
                 cache[serial - 100 + 0.5] = nil
             end
         end",
    );
    run(&mut lua, "churn(1000)");
    let warm = settle(&mut lua);
    for _ in 0..20 {
        run(&mut lua, "churn(10000)");
    }
    let after = settle(&mut lua);
    assert!(
        after < warm + 50_000,
        "200k churned keys bloated a 100-key table: {warm} -> {after} bytes"
    );
    assert_eq!(
        run(&mut lua, "local n = 0 for _ in pairs(cache) do n = n + 1 end return n"),
        ExVal::Integer(100)
    );
}

#[test]
fn string_key_churn_stays_bounded() {
    // Same as above with fresh string keys, so dead keys also pin their strings
    // until swept.
    let mut lua = Lua::new_with_standard();
    run(
        &mut lua,
        "cache = {} serial = 0
         function churn(n)
             for _ = 1, n do
                 serial = serial + 1
                 cache['k' .. serial] = serial
                 cache['k' .. (serial - 100)] = nil
             end
         end",
    );
    run(&mut lua, "churn(1000)");
    let warm = settle(&mut lua);
    for _ in 0..20 {
        run(&mut lua, "churn(10000)");
    }
    let after = settle(&mut lua);
    assert!(
        after < warm + 50_000,
        "200k churned string keys bloated a 100-key table: {warm} -> {after} bytes"
    );
}

// ── Garbage ──────────────────────────────────────────────────────────────────

#[test]
fn cyclic_tables_are_collected() {
    let mut lua = Lua::new_with_standard();
    run(
        &mut lua,
        "function ring(n)
             local first = {pad = {1, 2, 3, 4, 5, 6, 7, 8}}
             local prev = first
             for i = 1, n do
                 local t = {prev = prev, pad = {1, 2, 3, 4, 5, 6, 7, 8}}
                 prev.next = t
                 prev = t
             end
             prev.next = first
         end",
    );
    run(&mut lua, "ring(10)");
    let base = settle(&mut lua);
    for _ in 0..20 {
        run(&mut lua, "ring(5000)");
    }
    let after = settle(&mut lua);
    assert!(
        after < base + 10_000,
        "unreachable table cycles survived: {base} -> {after} bytes"
    );
}

#[test]
fn big_temporary_tables_trigger_collection() {
    // Without the buffers counted, a loop of big short-lived tables looks nearly
    // free to the collector's pacing and memory runs far ahead of what is live.
    let mut lua = Lua::new_with_standard();
    run(
        &mut lua,
        "function burst()
             local t = {}
             for i = 1, 20000 do t[i] = i end
             return #t
         end",
    );
    run(&mut lua, "burst()");
    let base = settle(&mut lua);
    let mut peak = 0;
    for _ in 0..200 {
        // `burst` runs inside one `run`; collection only paces between runs here
        run(&mut lua, "for _ = 1, 5 do burst() end");
        peak = peak.max(lua.allocated_bytes());
    }
    // 1000 dead arrays of 20k slots would be ~320 MB if never collected
    assert!(
        peak < base + 64_000_000,
        "collection fell behind table garbage: peak {peak} bytes over a {base} baseline"
    );
}

#[cfg(feature = "hot-swap")]
#[test]
fn repeated_hotswaps_stay_bounded() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    let src = |n: usize| {
        format!(
            "data = data or {{}}\n\
             function step(x) local t = {{}} for i = 1, 100 do t[i] = x + i + {n} end return #t end\n"
        )
    };
    lua.run(Some("game.lua"), &src(0), &mut c).unwrap();
    for n in 0..20 {
        lua.hotswap(Some("game.lua"), &src(n), &src(n + 1), &mut c).unwrap();
    }
    let warm = settle(&mut lua);
    for n in 20..220 {
        lua.hotswap(Some("game.lua"), &src(n), &src(n + 1), &mut c).unwrap();
        run(&mut lua, "step(1)");
    }
    let after = settle(&mut lua);
    assert!(
        after < warm + 50_000,
        "200 hot swaps of one function kept old code alive: {warm} -> {after} bytes"
    );
}

// ── Emptying a table during traversal ────────────────────────────────────────

#[test]
fn clearing_fields_inside_pairs_visits_them_all() {
    let mut lua = Lua::new_with_standard();
    let got = run(
        &mut lua,
        "local t = {}
         for i = 1, 100 do t['k' .. i] = i end
         local n = 0
         for k in pairs(t) do t[k] = nil n = n + 1 end
         return n * 1000 + (next(t) == nil and 1 or 0)",
    );
    assert_eq!(got, ExVal::Integer(100_001));
}

#[test]
fn clearing_a_mixed_table_inside_pairs_reaches_the_hash_part() {
    let mut lua = Lua::new_with_standard();
    let got = run(
        &mut lua,
        "local t = {1, 2, 3, x = 1, y = 2, z = 3}
         local n = 0
         for k in pairs(t) do t[k] = nil n = n + 1 end
         return n * 1000 + (next(t) == nil and 1 or 0)",
    );
    assert_eq!(got, ExVal::Integer(6_001));
}

#[test]
fn overwriting_fields_inside_pairs_visits_them_all() {
    // A full map used to rehash on any insert, even an overwrite, so the
    // traversal lost its place.
    let mut lua = Lua::new_with_standard();
    for n in [3, 7, 14, 28, 100] {
        let got = run(
            &mut lua,
            &format!(
                "local t = {{}}
                 for i = 1, {n} do t['k' .. i] = i end
                 for k, v in pairs(t) do t[k] = v * 10 end
                 local s = 0
                 for _, v in pairs(t) do s = s + v end
                 return s"
            ),
        );
        let want = (1..=n as i64).sum::<i64>() * 10;
        assert_eq!(got, ExVal::Integer(want), "{n} keys");
    }
}

#[test]
fn cleared_keys_read_as_absent() {
    let mut lua = Lua::new_with_standard();
    let got = run(
        &mut lua,
        "local t = {a = 1, [2.5] = 2, [10] = 3}
         t.a = nil t[2.5] = nil t[10] = nil
         g = 1 g = nil
         local n = 0 for _ in pairs(t) do n = n + 1 end
         return (t.a == nil and t[2.5] == nil and t[10] == nil and g == nil and n == 0)",
    );
    assert_eq!(got, ExVal::Bool(true));
}

#[test]
fn a_cleared_key_does_not_end_the_sequence_early() {
    // `t[4]` dead in the hash part must not stop `t[4] = v` from extending the
    // array, nor leave a hole when the array grows past it.
    let mut lua = Lua::new_with_standard();
    let got = run(
        &mut lua,
        "local t = {1, 2, 3}
         t[5] = 5 t[5] = nil
         t[4] = 4 t[5] = 5 t[6] = 6
         return #t",
    );
    assert_eq!(got, ExVal::Integer(6));
}
