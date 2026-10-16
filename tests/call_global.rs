//! `call_global` lets a host call optional global callbacks (`loop`, `draw`) directly,
//! getting `None` back when the script didn't define one.

use silt_lua::{Compiler, ExVal, Lua};

fn setup(code: &str) -> Lua {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    lua.run(Some("setup"), code, &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap();
    lua
}

fn get(lua: &mut Lua, name: &str) -> ExVal {
    let mut compiler = Compiler::new();
    lua.run(Some("get"), &format!("return {name}"), &mut compiler)
        .map_err(|e| e.to_string())
        .unwrap()
}

#[test]
fn missing_global_is_none() {
    let mut lua = setup("X = 1");
    assert!(lua.call_global("draw", ()).is_none());
    // Defined then cleared reads as missing too.
    let mut lua = setup("function draw() end draw = nil");
    assert!(lua.call_global("draw", ()).is_none());
}

#[test]
fn calls_the_function_with_args() {
    let mut lua = setup("W = 0 function draw(w, h) W = w * h end");
    let r = lua.call_global("draw", (3i64, 4i64)).unwrap();
    assert!(r.is_ok());
    assert_eq!(get(&mut lua, "W"), ExVal::Integer(12));
}

#[test]
fn closure_upvalues_survive() {
    let mut lua = setup(
        "local n = 0
         function loop() n = n + 1 end
         function count() return n end",
    );
    for _ in 0..5 {
        lua.call_global("loop", ()).unwrap().unwrap();
    }
    assert_eq!(lua.call_global("count", ()).unwrap().unwrap(), ExVal::Integer(5));
}

#[test]
fn locals_are_not_shifted_by_unit_args() {
    // `()` reaches the VM as one nil argument; a zero-parameter function must drop
    // it rather than let it occupy its first local's slot.
    let mut lua = setup("function f() local a = 1 local b = 2 return a * 10 + b end");
    assert_eq!(lua.call_global("f", ()).unwrap().unwrap(), ExVal::Integer(12));
}

#[test]
fn repeated_calls_do_not_drift() {
    let mut lua = setup("T = 0 function loop() local a, b, c = 1, 2, 3 T = T + a + b + c end");
    for _ in 0..1000 {
        lua.call_global("loop", ()).unwrap().unwrap();
    }
    assert_eq!(get(&mut lua, "T"), ExVal::Integer(6000));
}

#[test]
fn redefinition_is_picked_up() {
    // The "swap your own loop" state-machine pattern.
    let mut lua = setup(
        "S = ''
         function title() S = S .. 't' loop = game end
         function game() S = S .. 'g' end
         loop = title",
    );
    for _ in 0..3 {
        lua.call_global("loop", ()).unwrap().unwrap();
    }
    assert_eq!(get(&mut lua, "S"), ExVal::String("tgg".into()));
}

#[test]
fn runtime_error_is_reported() {
    let mut lua = setup("function loop() local t = nil return t.x end");
    let r = lua.call_global("loop", ()).unwrap();
    assert!(r.is_err());
    // The VM is still usable afterwards.
    let mut lua2 = setup("function loop() X = 1 end");
    assert!(lua2.call_global("loop", ()).unwrap().is_ok());
    let r = lua.call_global("loop", ());
    assert!(r.unwrap().is_err());
}

#[test]
fn non_callable_is_an_error() {
    let mut lua = setup("loop = 5");
    let r = lua.call_global("loop", ()).unwrap();
    assert!(r.is_err());
}

#[test]
fn native_global_is_callable() {
    let mut lua = setup("loop = print");
    assert!(lua.call_global("loop", ()).unwrap().is_ok());
}

