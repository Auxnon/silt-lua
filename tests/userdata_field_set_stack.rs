//! Regression: assigning to a userdata field must leave the VM stack balanced.
//!
//! `OpCode::TABLE_SET` has two branches. The `Table` branch runs through
//! `operate_table(.., Some(value))`, which pops the receiver + its `depth` keys
//! off the stack (`stack_count -= depth + 1`, rewinds `ip`, clears the slot). The
//! `UserData` branch previously called `set_field` and returned `Ok(())` with NO
//! stack cleanup, leaking `depth + 1` slots on every `ud.field = x`.
//!
//! A few such assignments desync the runtime stack from the compiler's fixed
//! local slots, so a local declared afterward reads a leaked value (a copy of the
//! userdata receiver) instead of its own initializer. This is exactly the bug
//! that made `local m = mus()` return the previously-mutated entity userdata in
//! petrichor after `ent.y = ..; ent.z = ..` ran earlier in the same frame.

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

/// The set itself works and a following field read is correct.
#[test]
fn field_set_then_read() {
    assert_eq!(
        run("local p = make_point() p.x = 5 p.x = 8 return p.x"),
        ExVal::Integer(8)
    );
}

/// A scalar local declared AFTER several field assignments must hold its own
/// initializer, not a leaked copy of the userdata receiver. Pre-fix this returned
/// the `Point` userdata (`ExVal::UserData`) because the leaked slots pushed the
/// runtime top past where the compiler placed `n`.
#[test]
fn local_after_field_sets_is_not_clobbered() {
    assert_eq!(
        run("local p = make_point() p.x = 1 p.x = 2 p.x = 3 p.x = 4 local n = 99 return n"),
        ExVal::Integer(99)
    );
}

/// The petrichor case: a native returning a fresh table, bound to a local right
/// after userdata field assignments. Pre-fix `type(t)` was `"userdata"`.
#[test]
fn table_return_after_field_sets_stays_a_table() {
    assert_eq!(
        run("local p = make_point() p.x = 1 p.x = 2 p.x = 3 p.x = 4 local t = make_tab() return type(t)"),
        ExVal::String("table".to_string())
    );
}

/// Field assignments inside a loop body must not accumulate a leak across
/// iterations (each iteration re-pushes the receiver for the assignment).
#[test]
fn field_sets_in_a_loop_do_not_drift() {
    assert_eq!(
        run("local p = make_point() for i = 1, 20 do p.x = i end local n = 7 return n"),
        ExVal::Integer(7)
    );
}
