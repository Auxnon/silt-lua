use silt_lua::userdata::UserDataMethods;
// use gc_arena::Mutation;
use silt_lua::error::ValueTypes;
use silt_lua::gc_arena::Mutation;
use silt_lua::userdata::{MetaMethod, UserData, UserDataFields};
use silt_lua::Lua;
use silt_lua::LuaError;
use silt_lua::Value;
use silt_lua::VM;
use silt_lua::{Compiler};

// Example UserData struct
struct Counter {
    count: i64,
}

impl Counter {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn increment(&mut self) -> i64 {
        self.count += 1;
        self.count
    }

    fn decrement(&mut self) -> i64 {
        self.count -= 1;
        self.count
    }

    fn get_count(&self) -> i64 {
        self.count
    }

    fn set_count(&mut self, value: i64) {
        self.count = value;
    }
}

impl UserData for Counter {
    fn type_name() -> &'static str {
        "Counter"
    }
    fn get_id(&self) -> usize {
        69
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(methods: &mut M) {
        methods.add_method_mut("increment", |_vm, _m, counter, _: ()| {
            if let Some(this) = counter {
                let value = this.increment();
                Ok(Value::Integer(value))
            } else {
                Err(LuaError::UDBadCall)
            }
        });

        methods.add_method_mut("decrement", |_vm, _m, counter, _: ()| {
            if let Some(this) = counter {
                let value = this.decrement();
                Ok(Value::Integer(value))
            } else {
                Err(LuaError::UDBadCall)
            }
        });

        methods.add_method_mut("reset", |_vm, _m, counter, _: ()| {
            if let Some(this) = counter {
                this.set_count(0);
                Ok(Value::Nil)
            } else {
                Err(LuaError::UDBadCall)
            }
        });

        // A method that takes a parameter — exercises `userdata:add(n)`.
        methods.add_method_mut("add", |_vm, _m, counter, n: i64| {
            if let Some(this) = counter {
                this.count += n;
                Ok(Value::Integer(this.count))
            } else {
                Err(LuaError::UDBadCall)
            }
        });

        methods.add_meta_method("__tostring", |_vm, _m, counter, _: ()| {
            if let Some(this) = counter {
                Ok(Value::String(format!("Counter({})", this.get_count())))
            } else {
                Err(LuaError::UDBadCall)
            }
        });

        methods.add_meta_method("__add", |_vm, _m, counter, value: Value| {
            if let Some(this) = counter {
                if let Value::Integer(n) = value {
                    Ok(Value::Integer(this.get_count() + n))
                } else {
                    Err(LuaError::ExpOpValueWithValue(
                        ValueTypes::UserData,
                        MetaMethod::Add,
                        value.to_error(),
                    ))
                }
            } else {
                Err(LuaError::UDBadCall)
            }
        });
    }

    fn add_fields<'v, F: UserDataFields<'v, Self>>(fields: &mut F) {
        fields.add_field_method_get("count", |_vm, _, counter| {
            Ok(Value::Integer(counter.get_count()))
        });

        fields.add_field_method_set("count", |_vm, _, counter, value| {
            if let Value::Integer(n) = value {
                counter.set_count(n);
                Ok(Value::Nil)
            } else {
                Err(LuaError::ExpOpValueWithValue(
                    ValueTypes::UserData,
                    MetaMethod::NewIndex,
                    value.to_error(),
                ))
            }
        });
    }
}

fn main() {
    // new_with_standard() so `print` and friends are available.
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_counter", make_userdata);
    });
    let res = lua.run(
        Some("counter userdata test"),
        r#"
         counter = make_counter()
         counter:increment()          -- self method, no args
         counter:add(10)              -- self method WITH a parameter
         print(counter.count)         -- field getter -> 11
         return counter:increment()   -- -> 12
         "#,
        &mut comp,
    );
    match res {
        Ok(o) => println!("{}", o),
        Err(ee) => {
            for e in ee.errors.iter() {
                println!("error: {}", e);
            }
        }
    }
}

pub fn make_userdata<'lua>(
    vm: &mut VM<'lua>,
    mc: &Mutation<'lua>,
    _args: Vec<Value<'lua>>,
) -> Result<Value<'lua>, LuaError> {
    // Register the Counter type with the VM
    // vm.register_userdata_type::<Counter>();

    // Create a Counter instance
    let counter = Counter::new();

    Ok(vm.create_userdata(mc, counter))

    // Now in Lua code, you can use:
    // counter:increment()
    // counter:reset()
    // print(counter.count)
    // counter.count = 10
    // print(counter + 5)
}
