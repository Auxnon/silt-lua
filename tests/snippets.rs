#![cfg(feature = "snippets")]
// Stage 1: extract a pure (no-upvalue) function into an arena-independent Snippet and run it in a
// fresh, isolated VM — proving the cross-arena copy. See SPEC-snippets.md.

use silt_lua::{value::ExVal, Compiler, Lua};

fn extract(src: &str) -> silt_lua::snippet::Snippet {
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.extract_snippet(None, src, &mut c)
        .expect("extraction should succeed")
}

#[test]
fn pure_function_round_trip() {
    let s = extract("return function(x) return x * 2 + 1 end");
    assert_eq!(s.proto.arity, 1);
    assert!(s.ref_globals.is_empty());
    assert!(s.value_globals.is_empty());
    assert_eq!(Lua::run_snippet(&s, vec![ExVal::Integer(20)]), Ok(ExVal::Integer(41)));
    assert_eq!(Lua::run_snippet(&s, vec![ExVal::Integer(5)]), Ok(ExVal::Integer(11)));
}

#[test]
fn build_then_call_repeatedly_in_one_vm() {
    let s = extract("return function(x) return x * x end");
    let mut vm = Lua::build_snippet(&s).expect("build should succeed");
    // the built VM is a standalone handle, runnable repeatedly
    assert_eq!(vm.call_snippet(vec![ExVal::Integer(3)]), Ok(ExVal::Integer(9)));
    assert_eq!(vm.call_snippet(vec![ExVal::Integer(10)]), Ok(ExVal::Integer(100)));
    assert_eq!(vm.call_snippet(vec![ExVal::Integer(12)]), Ok(ExVal::Integer(144)));
}

#[test]
fn multiple_args_and_locals() {
    let s = extract("return function(a, b) local c = a + b return c * c end");
    assert_eq!(s.proto.arity, 2);
    assert_eq!(
        Lua::run_snippet(&s, vec![ExVal::Integer(2), ExVal::Integer(3)]),
        Ok(ExVal::Integer(25))
    );
}

#[test]
fn stdlib_reference_resolves_from_target_vm() {
    // `math` is a reference global in the defining VM → not copied, resolved from the fresh VM,
    // which has its own `math`.
    let s = extract("return function(n) return math.floor(n) + 3 end");
    assert_eq!(s.ref_globals, vec!["math".to_string()]);
    assert_eq!(Lua::run_snippet(&s, vec![ExVal::Number(7.8)]), Ok(ExVal::Integer(10)));
}

#[test]
fn value_global_is_snapshotted() {
    // a user-defined value global the snippet reads is snapshotted by value into the snippet
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    lua.run(None, "SCALE = 4", &mut c).expect("setup run failed");
    let s = lua
        .extract_snippet(None, "return function(x) return x * SCALE end", &mut c)
        .unwrap();
    assert_eq!(s.value_globals.len(), 1);
    assert_eq!(s.value_globals[0].0, "SCALE");
    // runs in a fresh VM that never defined SCALE — the snapshot carries it
    assert_eq!(Lua::run_snippet(&s, vec![ExVal::Integer(5)]), Ok(ExVal::Integer(20)));
}

#[test]
fn upvalue_capture_is_rejected_for_now() {
    // stage-1 limitation: capturing an enclosing local is not yet supported
    let mut lua = Lua::new_with_standard();
    let mut c = Compiler::new();
    let r = lua.extract_snippet(
        None,
        "local factor = 3 return function(x) return x * factor end",
        &mut c,
    );
    assert!(r.is_err(), "upvalue-capturing snippet should be rejected in stage 1");
}
