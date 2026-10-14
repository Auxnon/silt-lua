//! Regression: a chained access that passes THROUGH a userdata, e.g.
//! `b.ent.flipped = v` or `b.ent.x`, where `b` is a table and `b.ent` a
//! userdata. The compiler emits one `TABLE_SET`/`TABLE_GET { depth: 2 }`, and
//! `operate_table` only walked tables, so the intermediate userdata raised
//! "Cannot perform table operations on a non-table value (userdata)".

use silt_lua::gc_arena::Mutation;
use silt_lua::userdata::{UserData, UserDataFields, UserDataMethods};
use silt_lua::{Compiler, ExVal, Lua, LuaError, Value, VM};

struct Point {
    x: i64,
}

impl UserData for Point {
    fn type_name() -> &'static str {
        "Point"
    }
    fn get_id(&self) -> usize {
        12
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(_m: &mut M) {}
    fn add_fields<'v, F: UserDataFields<'v, Self>>(f: &mut F) {
        f.add_field_method_get("x", |_vm, _mc, p| Ok(Value::Integer(p.x)));
        f.add_field_method_set("x", |_vm, _mc, p: &mut Self, x: i64| {
            p.x = x;
            Ok(Value::Nil)
        });
    }
}

fn make_point<'l>(
    vm: &mut VM<'l>,
    mc: &Mutation<'l>,
    _args: Vec<Value<'l>>,
) -> Result<Value<'l>, LuaError> {
    Ok(vm.create_userdata(mc, Point { x: 0 }))
}

fn make_tab<'l>(
    vm: &mut VM<'l>,
    mc: &Mutation<'l>,
    _args: Vec<Value<'l>>,
) -> Result<Value<'l>, LuaError> {
    let t = vm.raw_table();
    Ok(vm.wrap_table(mc, t))
}

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_point", make_point);
        vm.register_native_function(mc, "make_tab", make_tab);
    });
    lua.run(None, src, &mut comp)
        .map_err(|e| e.to_string())
        .unwrap()
}


#[test]
fn set_through_table_into_userdata() {
    assert_eq!(
        run("local b = { ent = make_point() } b.ent.x = 5 return b.ent.x"),
        ExVal::Integer(5)
    );
}

#[test]
fn get_through_deeper_chain() {
    assert_eq!(
        run("local t = { a = { ent = make_point() } } t.a.ent.x = 4 return t.a.ent.x + 1"),
        ExVal::Integer(5)
    );
}

#[test]
fn compound_assign_through_userdata() {
    assert_eq!(
        run("local b = { ent = make_point() } b.ent.x = 2 b.ent.x = b.ent.x * 3 return b.ent.x"),
        ExVal::Integer(6)
    );
}

/// Same stack-balance guarantee as the single-level case.
#[test]
fn chained_sets_keep_the_stack_balanced() {
    assert_eq!(
        run("local b = { ent = make_point() } for i = 1, 20 do b.ent.x = i end local n = 7 return n"),
        ExVal::Integer(7)
    );
}

/// The midnight-oil spawn_fireball shape: parameters, a constructor, then a
/// chained set into a freshly made userdata.
#[test]
fn spawn_fireball_shape() {
    assert_eq!(
        run("function spawn(x, f) local b = { x = x, ent = nil } b.ent = make_point() b.ent.x = f return b end local b = spawn(1, 9) return b.ent.x"),
        ExVal::Integer(9)
    );
}

#[test]
fn missing_link_still_errors() {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    let r = lua.run(None, "local b = {} b.ent.x = 1", &mut comp);
    assert!(r.is_err());
}
