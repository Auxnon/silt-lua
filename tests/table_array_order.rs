//! Regression: reading a Lua array into a Rust `[T; N]` (Table::to_array) must
//! preserve index order 1..N. It iterated the backing HashMap (hash order), so
//! `{a, b, c}` came out as an arbitrary cyclic rotation that varied per table —
//! e.g. entity `size = {w, d, h}` was randomly stretched on the wrong axis.

use silt_lua::gc_arena::Mutation;
use silt_lua::userdata::{UserData, UserDataFields, UserDataMethods};
use silt_lua::{Compiler, ExVal, Lua, LuaError, Value, VM};

struct Rec {
    v: [f64; 3],
}
impl UserData for Rec {
    fn type_name() -> &'static str {
        "Rec"
    }
    fn get_id(&self) -> usize {
        7
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(_: &mut M) {}
    fn add_fields<'v, F: UserDataFields<'v, Self>>(f: &mut F) {
        f.add_field_method_set("v", |_, _, t: &mut Self, val: [f64; 3]| {
            t.v = val;
            Ok(Value::Nil)
        });
        f.add_field_method_get("vx", |_, _, t| Ok(Value::Number(t.v[0])));
        f.add_field_method_get("vy", |_, _, t| Ok(Value::Number(t.v[1])));
        f.add_field_method_get("vz", |_, _, t| Ok(Value::Number(t.v[2])));
    }
}
fn make_rec<'l>(vm: &mut VM<'l>, mc: &Mutation<'l>, _: Vec<Value<'l>>) -> Result<Value<'l>, LuaError> {
    Ok(vm.create_userdata(mc, Rec { v: [0.0; 3] }))
}
fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_rec", make_rec);
    });
    lua.run(None, src, &mut comp).map_err(|e| e.to_string()).unwrap()
}

#[test]
fn array_field_set_preserves_order() {
    // Many tables in one run — the old hash-order bug rotated most of them.
    let src = "local s='' for i=1,8 do local r=make_rec() r.v={1.0,2.0,3.0} \
               s=s..r.vx..','..r.vy..','..r.vz..'|' end return s";
    assert_eq!(
        run(src),
        ExVal::String("1,2,3|1,2,3|1,2,3|1,2,3|1,2,3|1,2,3|1,2,3|1,2,3|".to_string())
    );
}
