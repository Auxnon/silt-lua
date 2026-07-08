//! `call_fn` / `call_with_params` must deliver runtime args to a loaded chunk (PLAN §2.14).
//! A chunk is a vararg function, so the args arrive as `...`.

use silt_lua::{Compiler, ExVal, Lua};

/// Load `code` and call it with the given tuple of args; return the result value.
fn call<T>(code: &str, args: T) -> ExVal
where
    T: for<'e> silt_lua::value::ToLuaMulti<'e>,
{
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let idx = lua
        .load_fn(&mut compiler, Some("cb"), code)
        .map_err(|e| e.to_string())
        .unwrap();
    lua.call_with_params(Some("cb"), idx, args)
        .map_err(|e| e.to_string())
        .unwrap()
}

#[test]
fn zero_arg_callback_still_works() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let idx = lua
        .load_fn(&mut compiler, Some("cb"), "return 42")
        .map_err(|e| e.to_string())
        .unwrap();
    let v = lua.call(Some("cb"), idx).map_err(|e| e.to_string()).unwrap();
    assert_eq!(v, ExVal::Integer(42));
}

#[test]
fn args_arrive_as_varargs() {
    // The `draw(w, h)`-on-resize case: the chunk reads its args via `...`.
    assert_eq!(call("local w, h = ... return w + h", (3i64, 4i64)), ExVal::Integer(7));
    assert_eq!(call("local w = ... return w", (7i64, 9i64)), ExVal::Integer(7));
}

#[test]
fn missing_args_are_nil() {
    // Fewer args than the chunk reads → the extras are nil (padded).
    assert_eq!(call("local a, b, c = ... return c", (1i64, 2i64)), ExVal::Nil);
}

#[test]
fn chunk_ignoring_args_is_unaffected() {
    assert_eq!(call("return 5", (99i64, 100i64)), ExVal::Integer(5));
}

#[test]
fn repeated_calls_do_not_drift() {
    // Game-loop style: call the same chunk many times with different args; the stack
    // must not accumulate, and each call must see only its own args.
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let idx = lua
        .load_fn(&mut compiler, Some("tick"), "local w, h = ... return w * h")
        .map_err(|e| e.to_string())
        .unwrap();
    for i in 1..=5i64 {
        let v = lua
            .call_with_params(Some("tick"), idx, (i, 10i64))
            .map_err(|e| e.to_string())
            .unwrap();
        assert_eq!(v, ExVal::Integer(i * 10));
    }
}
