# Benchmarks (excluded from normal CI)

This directory is the **performance** batch. It is deliberately kept out of the regular test
run so `cargo test` stays fast and deterministic. See `PLAN.md` §8.

## Two ways to measure

1. **In-process micro-benchmarks (Criterion)** — most precise, times only `Lua::run`:
   ```sh
   cargo bench
   ```
   `cargo bench` builds the `[[bench]]` target in `benches/interpreter.rs`. `cargo test` never
   touches it, so CI is unaffected. Criterion is a `dev-dependency`, so it is never compiled
   into the library or a release build.

2. **Wall-clock comparison vs PUC-Lua** — process-level, compares against the reference `lua`:
   ```sh
   benchmarks/compare.sh           # best-of-3 by default; REPS=5 benchmarks/compare.sh
   ```
   Runs every `*.lua` here under both `./target/release/silt <file>` and the reference `lua`
   (or `lua5.4`/`luajit`) found on `PATH`, printing a side-by-side table with the silt/lua ratio.
   If no reference Lua is installed, it prints silt-only timings.

## Workloads
- `forloop.lua` — tight numeric `for` arithmetic.
- `whileloop.lua` — `while` loop with an inner branch.
- `fib.lua` — naive recursive Fibonacci (call/return + arithmetic).
- `closure_counter.lua` — closures capturing upvalues (the per-entity update pattern).
  ⚠️ Currently crashes silt — blocked on `PLAN.md` §1.1. Kept here for when it is fixed.
- `strconcat.lua` — allocation-heavy string building.
- `tableops.lua` — array fill + sum (table read/write).

## On using this as a regression gate (later)
Per the project prompt, this batch *may* graduate into a regression test once timings are
stable in CI. Do **not** wire it into PR-blocking CI yet — interpreter micro-benchmarks are
notoriously noisy on shared CI runners. The recommended path is a separate **scheduled** job
that runs `cargo bench`, compares against a committed Criterion baseline
(`cargo bench -- --save-baseline main` then `--baseline main`), and only reports/alerts on a
regression beyond a generous threshold (e.g. >10%).
