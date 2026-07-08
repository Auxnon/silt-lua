//! Performance benchmarks for the silt-lua interpreter.
//!
//! These are **excluded from normal CI**: `cargo test` does not build or run `[[bench]]`
//! targets — they run only via `cargo bench`. See PLAN.md §8.
//!
//! Each benchmark times only the `Lua::run` call (compile + execute) so we measure the
//! interpreter, not process startup. To compare against PUC-Lua, run `benchmarks/compare.sh`,
//! which times the same `.lua` scripts under the reference `lua` interpreter.
//!
//! Workloads scaled to complete quickly under Criterion; the standalone `benchmarks/*.lua`
//! scripts use larger N for wall-clock comparison against PUC-Lua.
//!
//! NOTE: a closure/upvalue benchmark is intentionally omitted until PLAN.md §1.1 is fixed
//! (capturing closures currently panic the compiler). Re-add `closure_counter` then.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use silt_lua::{Compiler, Lua};

fn run(source: &str) {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    let _ = lua.run(None, black_box(source), &mut compiler);
}

const FOR_LOOP: &str = r#"
    local sum = 0
    for i = 1, 1000000 do
        sum = sum + i
    end
    return sum
"#;

const WHILE_LOOP: &str = r#"
    local i = 0
    local acc = 0
    while i < 1000000 do
        i = i + 1
        acc = acc + i
    end
    return acc
"#;

const FIB: &str = r#"
    function fib(n)
        if n < 2 then return n end
        return fib(n - 1) + fib(n - 2)
    end
    return fib(28)
"#;

const STR_CONCAT: &str = r#"
    local s = ""
    for i = 1, 10000 do
        s = s .. "x"
    end
    return #s
"#;

const TABLE_OPS: &str = r#"
    local t = {}
    for i = 1, 200000 do t[i] = i * 2 end
    local sum = 0
    for i = 1, 200000 do sum = sum + t[i] end
    return sum
"#;

fn benches(c: &mut Criterion) {
    c.bench_function("for_loop_1e6", |b| b.iter(|| run(FOR_LOOP)));
    c.bench_function("while_loop_1e6", |b| b.iter(|| run(WHILE_LOOP)));
    c.bench_function("fib_28_recursion", |b| b.iter(|| run(FIB)));
    c.bench_function("string_concat_1e4", |b| b.iter(|| run(STR_CONCAT)));
    c.bench_function("table_rw_2e5", |b| b.iter(|| run(TABLE_OPS)));
}

criterion_group!(g, benches);
criterion_main!(g);
