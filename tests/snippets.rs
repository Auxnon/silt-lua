#![cfg(feature = "snippets")]
//! Snippets: extract a function into an arena-independent `Snippet`, ship it to another thread,
//! build it into its own isolated VM and call it. See SPEC-snippets.md.
//!   cargo test --features snippets --test snippets

use silt_lua::{
    snippet::{Snippet, SnippetError, SnippetVm},
    value::ExVal,
    Compiler, Lua,
};
use std::sync::{Arc, Mutex};

fn extract(src: &str) -> Snippet {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.extract_snippet(None, src, &mut c)
        .expect("extraction should succeed")
}

fn int(i: i64) -> ExVal {
    ExVal::Integer(i)
}

// ---------------------------------------------------------------- stage 1: pure functions

#[test]
fn pure_function_round_trip() {
    let s = extract("return function(x) return x * 2 + 1 end");
    assert_eq!(s.arity(), 1);
    assert!(s.ref_globals.is_empty());
    assert!(s.value_globals.is_empty());
    assert!(s.upvalues.is_empty());
    assert_eq!(Lua::run_snippet(&s, vec![int(20)]), Ok(int(41)));
    assert_eq!(Lua::run_snippet(&s, vec![int(5)]), Ok(int(11)));
}

#[test]
fn build_then_call_repeatedly_in_one_vm() {
    let s = extract("return function(x) return x * x end");
    let mut vm = Lua::build_snippet(&s).expect("build should succeed");
    assert_eq!(vm.call(&[int(3)]), Ok(int(9)));
    assert_eq!(vm.call(&[int(10)]), Ok(int(100)));
    assert_eq!(vm.call(&[int(12)]), Ok(int(144)));
}

#[test]
fn multiple_args_and_locals() {
    let s = extract("return function(a, b) local c = a + b return c * c end");
    assert_eq!(s.arity(), 2);
    assert_eq!(Lua::run_snippet(&s, vec![int(2), int(3)]), Ok(int(25)));
}

#[test]
fn stdlib_reference_resolves_from_target_vm() {
    // `math` is a reference global in the defining VM → not copied, resolved from the fresh VM,
    // which has its own `math`.
    let s = extract("return function(n) return math.floor(n) + 3 end");
    assert_eq!(s.ref_globals, vec!["math".to_string()]);
    assert_eq!(Lua::run_snippet(&s, vec![ExVal::Number(7.8)]), Ok(int(10)));
}

#[test]
fn value_global_is_snapshotted() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(None, "SCALE = 4", &mut c).expect("setup run failed");
    let s = lua
        .extract_snippet(None, "return function(x) return x * SCALE end", &mut c)
        .unwrap();
    assert_eq!(s.value_globals.len(), 1);
    assert_eq!(s.value_globals[0].0, "SCALE");
    // runs in a fresh VM that never defined SCALE — the snapshot carries it
    assert_eq!(Lua::run_snippet(&s, vec![int(5)]), Ok(int(20)));
}

#[test]
fn pure_data_global_table_is_deep_copied() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(None, "CFG = { gain = 2, name = 'x', harmonics = {1, 0.5, 0.25} }", &mut c)
        .unwrap();
    let s = lua
        .extract_snippet(
            None,
            "return function(i) return CFG.gain * CFG.harmonics[i] end",
            &mut c,
        )
        .unwrap();
    assert_eq!(s.value_globals.len(), 1);
    assert_eq!(Lua::run_snippet(&s, vec![int(2)]), Ok(ExVal::Number(1.0)));
}

// ---------------------------------------------------------------- stage 2: captured upvalues

#[test]
fn value_upvalue_is_captured_by_value() {
    let s = extract("local factor = 3 return function(x) return x * factor end");
    assert_eq!(s.upvalues, vec![int(3)]);
    assert_eq!(Lua::run_snippet(&s, vec![int(7)]), Ok(int(21)));
}

#[test]
fn several_upvalues_including_strings_and_tables() {
    let s = extract(
        "local a, b = 10, 'hi'
         local t = { 1, 2, 3 }
         return function() return b .. (a + t[3]) end",
    );
    assert_eq!(s.upvalues.len(), 3);
    assert_eq!(
        Lua::run_snippet(&s, vec![]),
        Ok(ExVal::String("hi13".to_string()))
    );
}

#[test]
fn upvalue_is_a_snapshot_not_shared_state() {
    // mutating the captured copy inside the snippet persists across calls in the snippet VM,
    // but never reaches the defining VM (there is no shared cell)
    let s = extract("local n = 0 return function() n = n + 1 return n end");
    let mut vm = Lua::build_snippet(&s).unwrap();
    assert_eq!(vm.call(&[]), Ok(int(1)));
    assert_eq!(vm.call(&[]), Ok(int(2)));
    // a second build starts from the snapshot again
    let mut vm2 = Lua::build_snippet(&s).unwrap();
    assert_eq!(vm2.call(&[]), Ok(int(1)));
}

#[test]
fn reference_upvalue_is_rejected() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    let r = lua.extract_snippet(
        None,
        "local helper = function(x) return x end return function(x) return helper(x) end",
        &mut c,
    );
    let msg = r.err().expect("closure upvalue must be rejected");
    assert!(msg.contains("captured upvalue #0"), "{msg}");
    // a table that holds a function is a reference too
    let r = lua.extract_snippet(
        None,
        "local t = { f = print } return function() return t.f end",
        &mut c,
    );
    assert!(r.is_err());
}

// ---------------------------------------------------------------- nested functions

#[test]
fn nested_functions_inside_the_snippet_work() {
    // an inner closure capturing the snippet's own local is created at run time by the
    // normal CLOSURE machinery (its upvalue is internal, not an escaping one)
    let s = extract(
        "return function(n)
            local acc = 0
            local function add(v) acc = acc + v end
            for i = 1, n do add(i) end
            return acc
         end",
    );
    assert!(s.upvalues.is_empty());
    assert_eq!(Lua::run_snippet(&s, vec![int(10)]), Ok(int(55)));
}

#[test]
fn snippet_returning_a_closure_factory_result() {
    let s = extract(
        "local base = 100
         return function(k)
            local function mk(m) return function(x) return base + m * x end end
            return mk(k)(2)
         end",
    );
    assert_eq!(Lua::run_snippet(&s, vec![int(5)]), Ok(int(110)));
}

#[test]
fn variadic_snippet() {
    let s = extract("return function(...) return select('#', ...) end");
    assert!(s.is_variadic());
    assert_eq!(Lua::run_snippet(&s, vec![int(1), int(2), int(3)]), Ok(int(3)));
}

// ---------------------------------------------------------------- isolation

#[test]
fn snippet_globals_do_not_leak_back_to_the_defining_vm() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(None, "COUNT = 1", &mut c).unwrap();
    let s = lua
        .extract_snippet(None, "return function() COUNT = COUNT + 1 return COUNT end", &mut c)
        .unwrap();
    let mut vm = Lua::build_snippet(&s).unwrap();
    assert_eq!(vm.call(&[]), Ok(int(2)));
    assert_eq!(vm.call(&[]), Ok(int(3)));
    // defining VM untouched
    assert_eq!(lua.run(None, "return COUNT", &mut c).unwrap(), int(1));
}

#[test]
fn extract_requires_a_function() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    let r = lua.extract_snippet(None, "return 42", &mut c);
    assert_eq!(r.err().unwrap(), SnippetError::NotAFunction.to_string());
}

// ---------------------------------------------------------------- errors are tagged

#[test]
fn runtime_errors_are_tagged_and_dropped_references_are_named() {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    // `helper` is a live function global in the defining VM: a reference, not copied
    lua.run(None, "function helper(x) return x * 2 end", &mut c)
        .unwrap();
    let s = lua
        .extract_snippet(None, "return function(x) return helper(x) end", &mut c)
        .unwrap();
    assert_eq!(s.ref_globals, vec!["helper".to_string()]);
    let err = Lua::run_snippet(&s, vec![int(1)]).err().expect("must fail");
    assert!(err.starts_with("[snippet]"), "{err}");
    assert!(err.contains("'helper'"), "{err}");
    assert!(err.contains("not copied into the snippet"), "{err}");
}

#[test]
fn plain_runtime_error_is_tagged_without_a_hint() {
    let s = extract("return function(x) return x + {} end");
    let err = Lua::run_snippet(&s, vec![int(1)]).err().expect("must fail");
    assert!(err.starts_with("[snippet]"), "{err}");
    assert!(!err.contains("note:"), "{err}");
}

// ---------------------------------------------------------------- the host-native path

#[test]
fn host_native_extracts_a_snippet_from_a_closure_argument() {
    // This is the real embedding shape: `instr(1, function(p) ... end)` — the native receives
    // the closure Value and extracts while the defining frame is still live (open upvalues).
    let outbox: Arc<Mutex<Option<Snippet>>> = Arc::new(Mutex::new(None));
    let mut lua = Lua::new_with_standard();
    lua.enter(|vm, mc| {
        let sink = outbox.clone();
        vm.register_native_function(
            mc,
            "capture",
            move |vm: &mut silt_lua::VM, _mc, f: silt_lua::value::Value| {
                let s = Snippet::extract(vm, &f)?; // SnippetError -> SiltError
                *sink.lock().unwrap() = Some(s);
                Ok(true)
            },
        );
    });
    let mut c = Compiler::new();
    lua.run(
        None,
        "local gain = 0.25
         local offset = 1
         capture(function(x) return x * gain + offset end)
         gain = 999 -- changes after capture must not matter",
        &mut c,
    )
    .unwrap();
    let s = outbox.lock().unwrap().take().expect("native stored a snippet");
    assert_eq!(s.upvalues, vec![ExVal::Number(0.25), int(1)]);
    assert_eq!(Lua::run_snippet(&s, vec![int(8)]), Ok(ExVal::Number(3.0)));
}

// ---------------------------------------------------------------- cross-thread

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn snippet_is_send_and_sync() {
    assert_send_sync::<Snippet>();
}

#[test]
fn snippet_runs_on_another_thread() {
    let s = extract(
        "local gain = 0.5
         return function(phase) return math.sin(phase) * gain end",
    );
    let handle = std::thread::spawn(move || {
        let mut vm: SnippetVm = Lua::build_snippet(&s).unwrap();
        let mut out = vec![0f32; 480];
        let rate = 48_000.0;
        let freq = 440.0;
        vm.render(&mut out, |i| {
            [2.0 * std::f64::consts::PI * freq * i as f64 / rate]
        })
        .unwrap();
        out
    });
    let out = handle.join().unwrap();
    for (i, o) in out.iter().enumerate() {
        let expect = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 48_000.0).sin() * 0.5;
        assert!((*o as f64 - expect).abs() < 1e-6, "sample {i}: {o} vs {expect}");
    }
}

#[test]
fn one_snippet_many_threads() {
    let s = extract("return function(a, b) return a * b end");
    let s = Arc::new(s);
    let handles: Vec<_> = (0..4)
        .map(|t| {
            let s = s.clone();
            std::thread::spawn(move || {
                let mut vm = Lua::build_snippet(&s).unwrap();
                (0..1000)
                    .map(|i| vm.call_f64(&[i as f64, t as f64]).unwrap())
                    .sum::<f64>()
            })
        })
        .collect();
    for (t, h) in handles.into_iter().enumerate() {
        let sum = h.join().unwrap();
        assert_eq!(sum, (0..1000).map(|i| (i * t) as f64).sum::<f64>());
    }
}

#[test]
fn call_f64_fast_path() {
    let s = extract("return function(x, y) return x + y end");
    let mut vm = Lua::build_snippet(&s).unwrap();
    assert_eq!(vm.call_f64(&[1.5, 2.0]), Ok(3.5));
    // integer results are widened
    let s = extract("return function() return 7 end");
    let mut vm = Lua::build_snippet(&s).unwrap();
    assert_eq!(vm.call_f64(&[]), Ok(7.0));
    // non-numeric results are an error, not a silent 0
    let s = extract("return function() return 'nope' end");
    let mut vm = Lua::build_snippet(&s).unwrap();
    let err = vm.call_f64(&[]).err().unwrap();
    assert!(err.contains("number was expected"), "{err}");
}

#[test]
fn snippet_vm_accepts_host_natives() {
    // the target can provide references the snippet expects (`emit` was nil-resolved)
    let s = extract("return function(x) return emit(x) end");
    assert_eq!(s.ref_globals, vec!["emit".to_string()]);
    let mut vm = Lua::build_snippet(&s).unwrap();
    vm.lua().enter(|vm, mc| {
        vm.register_native_function(mc, "emit", |_vm: &mut silt_lua::VM, _mc, x: f64| {
            Ok(x * 10.0)
        });
    });
    assert_eq!(vm.call_f64(&[4.0]), Ok(40.0));
}

#[test]
fn plain_run_does_not_retain_the_returned_value() {
    // `last_return` exists so `extract_snippet` can read a returned closure; an ordinary
    // `run` must not keep the chunk's result alive until the next run.
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(None, "return 1", &mut c).unwrap();
    lua.collect_full();
    lua.collect_full();
    let baseline = lua.allocated_bytes();
    lua.run(
        None,
        "local t = {} for i = 1, 20000 do t[i] = {i} end return t",
        &mut c,
    )
    .unwrap();
    lua.collect_full();
    lua.collect_full();
    let after = lua.allocated_bytes();
    assert!(
        after < baseline + 100_000,
        "returned table retained after run: {baseline} -> {after} bytes"
    );
}
