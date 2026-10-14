//! The userdata registry is keyed by Rust `TypeId` and caches one callable
//! `Value` per method at registration time (instead of allocating a fresh
//! `Gc<WrappedFn>` on every `ud:m()`). Those cached values must survive GC, and
//! two Rust types that happen to report the same `type_name()` must not share a
//! method table.

use silt_lua::gc_arena::Mutation;
use silt_lua::userdata::{UserData, UserDataFields, UserDataMethods};
use silt_lua::{Compiler, ExVal, Lua, LuaError, Value, VM};

struct Counter {
    n: i64,
}
impl UserData for Counter {
    fn type_name() -> &'static str {
        "Thing"
    }
    fn get_id(&self) -> usize {
        1
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(m: &mut M) {
        m.add_method_mut("bump", |_, _, this: Option<&mut Self>, by: i64| {
            if let Some(t) = this {
                t.n += by;
            }
            Ok(Value::Nil)
        });
        m.add_method("get", |_, _, this: Option<&mut Self>, _: ()| {
            Ok(Value::Integer(this.map(|t| t.n).unwrap_or(-1)))
        });
    }
    fn add_fields<'v, F: UserDataFields<'v, Self>>(f: &mut F) {
        f.add_field_method_get("n", |_, _, t| Ok(Value::Integer(t.n)));
        f.add_field_method_set("n", |_, _, t: &mut Self, v: i64| {
            t.n = v;
            Ok(Value::Nil)
        });
    }
}

/// Same `type_name()` as `Counter`, different Rust type and different methods.
struct Label {
    s: String,
}
impl UserData for Label {
    fn type_name() -> &'static str {
        "Thing"
    }
    fn get_id(&self) -> usize {
        2
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(m: &mut M) {
        m.add_method("text", |_, _, this: Option<&mut Self>, _: ()| {
            Ok(Value::String(this.map(|t| t.s.clone()).unwrap_or_default()))
        });
    }
}

fn make_counter<'l>(
    vm: &mut VM<'l>,
    mc: &Mutation<'l>,
    _: Vec<Value<'l>>,
) -> Result<Value<'l>, LuaError> {
    Ok(vm.create_userdata(mc, Counter { n: 0 }))
}
fn make_label<'l>(
    vm: &mut VM<'l>,
    mc: &Mutation<'l>,
    _: Vec<Value<'l>>,
) -> Result<Value<'l>, LuaError> {
    Ok(vm.create_userdata(mc, Label { s: "hi".into() }))
}

fn lua() -> (Lua, Compiler) {
    let mut lua = Lua::new_with_standard();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_counter", make_counter);
        vm.register_native_function(mc, "make_label", make_label);
    });
    (lua, Compiler::new())
}

#[test]
fn methods_and_fields_work() {
    let (mut l, mut c) = lua();
    let src = "local t = make_counter() t:bump(5) t.n = t.n * 2 t:bump(1) return t:get()";
    assert_eq!(
        l.run(None, src, &mut c).map_err(|e| e.to_string()),
        Ok(ExVal::Integer(11))
    );
}

#[test]
fn cached_method_values_survive_garbage_collection() {
    // Many runs, each allocating garbage and calling methods, with explicit
    // collections in between. If the registry's cached method `Value`s were not
    // traced they would be freed and the next `t:bump` would hit dead memory.
    let (mut l, _) = lua();
    for i in 0..200 {
        // Fresh compiler per run: reusing one across runs carries local-slot
        // state over (pre-existing bug, tracked separately).
        let mut c = Compiler::new();
        let src = format!(
            "local junk = {{}} for i = 1, 200 do junk[i] = {{ tostring(i) }} end \
             local t = make_counter() t:bump({i}) return t:get()"
        );
        assert_eq!(
            l.run(None, &src, &mut c).map_err(|e| e.to_string()),
            Ok(ExVal::Integer(i))
        );
        l.collect_full();
    }
}

#[test]
fn same_type_name_different_rust_types_do_not_collide() {
    let (mut l, mut c) = lua();
    // Register Counter first (claims the name "Thing"), then use Label.
    let src =
        "local a = make_counter() a:bump(3) local b = make_label() return b:text() .. a:get()";
    assert_eq!(
        l.run(None, src, &mut c).map_err(|e| e.to_string()),
        Ok(ExVal::String("hi3".to_string()))
    );
}

#[test]
fn calling_a_method_of_the_other_type_is_an_error_not_a_crash() {
    let (mut l, mut c) = lua();
    let err = l
        .run(None, "local b = make_label() return b:bump(1)", &mut c)
        .map_err(|e| e.to_string())
        .unwrap_err();
    assert!(!err.is_empty());
}
