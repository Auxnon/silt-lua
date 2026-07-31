# Spec: hybrid array + hash `Table` (PUC-Lua layout)

Status: **proposed** · Feature gate: `array-table` (default-on candidate once proven) ·
Supersedes the stale `copilot/explore-dual-array-hashmap` branch (do **not** cherry-pick — it
lacks `remove`/border/migration and predates this release's `table.*` fixes).

## Why do this at all (post-hash-fix)

The discriminant-only `Hash` bug is fixed (`1596318`), so table access is already O(1) and an
array-read loop is ~16× faster than before. So this is **not** primarily a speed change anymore.
The reasons PUC-Lua keeps a dedicated array part — the ones a "straight hashmap" genuinely cannot
match — are:

1. **Memory density.** A `HashMap<Value, Value>` entry stores the *key* (a 32-byte `Value`), the
   *value* (another 32 bytes), plus bucket/control bytes and load-factor slack (~1.5–2× over-alloc).
   A dense 10k-element array costs ~640 KB+ as a hashmap vs ~320 KB as a `Vec<Value>` (values only,
   no stored keys) — roughly a **2–4× reduction**, and less allocator pressure/GC scanning. For a
   game engine pushing vertex/particle/tile arrays this is the dominant win.
2. **Cache-friendly sequential access.** `ipairs`, `#`, `table.concat/insert/remove/unpack`, and
   `for i=1,#t` walk contiguous memory instead of chasing hash buckets in arbitrary order.
3. **Cheaper, well-defined `#` border.** The array part gives an unambiguous border via a binary
   search over its tail (`O(log n)`), instead of today's "hashmap entry count / counter" heuristic
   (a documented deviation in `PLAN.md §4`).
4. **No hashing/probing on sequence keys at all.** Even a perfect hash still hashes the integer and
   probes; the array part is a direct bounds-checked offset (~0.5 ns vs ~7.5 ns in isolation).

Honest expectation: **~10–15 %** wall-clock on array-heavy interpreted loops (the hashmap lookup is
only ~7 ns of a ~50 ns interpreted read), and a **2–4× memory** cut on large sequences. Ship it for
the memory + semantics, not for a headline speed number.

## Data structure

```rust
pub struct Table<'v> {
    array: Vec<Value<'v>>,                  // dense 1..=array.len(), 0-indexed internally
    hash:  HashMap<Value<'v>, Value<'v>>,   // everything else (sparse ints, strings, …)
    meta:  Option<Value<'v>>,
    id:    usize,
    // NOTE: drop the old `counter`; the border derives from `array` + a migration check.
}
```

Invariant: `array` never holds a trailing `Nil`. `array.last()` is always non-`Nil` (or the array is
empty). Integer key `k` lives in `array[k-1]` iff `1 <= k <= array.len()`; all other keys live in
`hash`. This invariant is what makes `#` and iteration correct — it's exactly what the abandoned
branch was missing.

## Core operations

### `get(k)`
- `Value::Integer(i)` with `1 <= i <= array.len()` → `array[i-1]` (never `Nil` by invariant).
- otherwise → `hash.get(k)`.

### `set(k, v)`
Route by key:
- **Integer in array range** (`1 <= i <= array.len()`):
  - `v != Nil` → overwrite `array[i-1]`.
  - `v == Nil` **and `i == array.len()`** → `pop`, then pop any further trailing `Nil`s created
    (keeps the no-trailing-Nil invariant, shrinks the border).
  - `v == Nil` and `i < array.len()` → this punches a hole. Split: truncate `array` at `i-1` and
    migrate the popped tail (`i+1 .. old_len`) into `hash`, then the slot `i` is simply absent.
    (Matches Lua: `t[k]=nil` mid-array turns the suffix into hash entries; `#t` may then legally
    report any border.)
- **Integer just past the end** (`i == array.len() + 1`, `v != Nil`): push to `array`, then
  **absorb** — repeatedly check `hash` for `array.len()+1` and move it over until the next key is
  missing. This is the migration the old branch never did; it's what lets `t[2]=b; t[1]=a` (built
  out of order) collapse into a dense array.
- **Everything else** (string, sparse int, bool, non-append integer, `v == Nil` on absent key):
  `hash.insert`/`remove`.

### `push(v)` (used by `table.insert` append + constructor array part)
`array.push(v)` then run the same **absorb** step. O(1) amortized.

### `insert_at(pos, v)` / `remove_at(pos)` (positional `table.insert`/`table.remove`)
Preserve this release's shift semantics. If `pos` is within `array`, `Vec::insert`/`Vec::remove`
(shift in contiguous memory — cheaper than today's per-key hash rewrites). If it straddles the
array/hash boundary, fall back to the current key-shift loop over the hash part.

### `len()` → border
Return `array.len()` when `hash` has no integer key `== array.len()+1`; otherwise binary-search the
combined space like PUC-Lua (`luaH_getn`). For the common all-array case it's `array.len()`, O(1).

### Iteration (`next_entry` / `pairs` / `TableIterator`)
Yield the array part first, indices `1..=array.len()` in order (this makes `pairs` order stable and
positional for the sequence part — a nice property tests can rely on), then drain `hash`. `next`
must map a caller-supplied key back to "where were we": if the key is an integer `<= array.len()`,
continue in the array; once past `array.len()`, switch to a `hash` iterator. Keep a resumable cursor
rather than re-scanning (the current impl re-derives position — preserve or improve, don't regress).

## Migration threshold / guards
- `MAX_ARRAY_SIZE` cap (branch used `1024`) prevents a single `t[10_000_000]=x` from allocating a
  10M-slot `Vec`. Above the cap, large integer keys go to `hash`. Keep a cap but make it generous
  (e.g. grow the array only when the resulting density `used/capacity` would stay > 0.5, PUC-Lua's
  rehash rule) so genuine large arrays still get the array part.
- The **absorb** and **hole-split** steps are the only migrations; both are amortized O(1) per op in
  the common append/pop pattern. Avoid migrating on every random `set`.

## Interaction points to update (grep before you start)
- `src/lua.rs`: `TABLE_GET`, `TABLE_GET_BY_CONSTANT`, `TABLE_SET`, `METHOD_GET`, `LENGTH`, the
  generic-for `next` path, and `wrap_table`.
- `src/standard.rs`: `table_insert`, `table_remove`, `table_concat`, `table_unpack`, `lua_pairs`,
  `lua_ipairs`, `next`.
- `src/table.rs`: all of `get*/set/insert/remove/push/len/is_empty/next_entry/to_vec/to_array/
  to_exval/iter`. These already funnel through a small accessor set — that's why this is contained.
- `ExTable` (the external mirror): give it the same array+hash split *or* keep it flat and only
  split the internal `Table` (simpler; `to_exval` flattens). Prefer keeping `ExVal`/`ExTable` flat
  to avoid touching the embedding API and serde surface.

## Testing
- **Parity**: run the existing `tests/tables.rs` unchanged — same results.
- **Border/holes**: `t={1,2,3}; t[3]=nil` → `#t==2`; `t[2]=nil` mid-array behavior; `t[#t+1]=x`.
- **Out-of-order build**: `t[3]=c; t[1]=a; t[2]=b` → dense array, `#t==3`, `ipairs` yields 3.
- **Migration**: large sparse key stays in hash; `MAX_ARRAY_SIZE` boundary; density rule.
- **Iteration**: `pairs` visits array part in index order then hash; `next` resumes correctly across
  the array→hash boundary; `ipairs` stops at first `nil`.
- **Memory** (optional, `dev` bench): allocate a 100k-int table, compare RSS array vs hash build.
- **Perf** (informational, not a CI gate — flaky): the N-scaling bench from the hash-fix
  investigation should stay flat and shave the ~7 ns hashmap slice on the array path.

## Rollout
1. Land behind `array-table` feature, default-off.
2. Green all suites with the feature on **and** off (the flat-hash path stays as the fallback).
3. Add the border/migration/iteration tests above.
4. Bench memory on a large-array workload to confirm the 2–4× claim on real data.
5. Flip default-on for a release once soaked, keeping the flag as an escape hatch.
```
