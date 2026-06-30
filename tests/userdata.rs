//! UserData method dispatch — especially colon self-calls `ud:method(param)`.

use silt_lua::gc_arena::Mutation;
use silt_lua::userdata::{UserData, UserDataFields, UserDataMethods};
use silt_lua::{Compiler, ExVal, Lua, LuaError, Value, VM};

struct Acc {
    total: i64,
}

impl UserData for Acc {
    fn type_name() -> &'static str {
        "Acc"
    }
    fn get_id(&self) -> usize {
        7
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(m: &mut M) {
        // No-arg self method.
        m.add_method_mut("bump", |_vm, _mc, this, _: ()| match this {
            Some(acc) => {
                acc.total += 1;
                Ok(Value::Integer(acc.total))
            }
            None => Err(LuaError::UDBadCall),
        });
        // Self method WITH a parameter — the case being fixed.
        m.add_method_mut("add", |_vm, _mc, this, n: i64| match this {
            Some(acc) => {
                acc.total += n;
                Ok(Value::Integer(acc.total))
            }
            None => Err(LuaError::UDBadCall),
        });
    }
    fn add_fields<'v, F: UserDataFields<'v, Self>>(f: &mut F) {
        f.add_field_method_get("total", |_vm, _mc, acc| Ok(Value::Integer(acc.total)));
    }
}

fn make_acc<'l>(
    vm: &mut VM<'l>,
    mc: &Mutation<'l>,
    _args: Vec<Value<'l>>,
) -> Result<Value<'l>, LuaError> {
    Ok(vm.create_userdata(mc, Acc { total: 0 }))
}

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_acc", make_acc);
    });
    lua.run(None, src, &mut comp)
        .map_err(|e| e.to_string())
        .unwrap()
}

#[test]
fn colon_method_no_args() {
    assert_eq!(
        run("local a = make_acc() a:bump() return a:bump()"),
        ExVal::Integer(2)
    );
}

#[test]
fn colon_method_with_param() {
    // 0 +1 (bump) +10 (add) then return a:add(5) -> 16
    assert_eq!(
        run("local a = make_acc() a:bump() a:add(10) return a:add(5)"),
        ExVal::Integer(16)
    );
}

#[test]
fn field_get_after_colon_calls() {
    assert_eq!(
        run("local a = make_acc() a:add(40) a:bump() return a.total"),
        ExVal::Integer(41)
    );
}
