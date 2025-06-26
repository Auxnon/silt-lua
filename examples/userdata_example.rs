// use gc_arena::Mutation;
use silt_lua::Compiler;
use silt_lua::LuaError;
use silt_lua::userdata::{UserDataMethods};
use silt_lua::gc_arena::Mutation;

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
    fn get_type() -> &'static str {
        "Counter"
    }

    fn add_methods<'v, M: UserDataMethods<'v, Self>>(methods: &mut M) {
        methods.add_method("increment", |_vm, counter, _value| {
            let value = counter.increment();
            Ok(Value::Integer(value))
        });

        methods.add_method("decrement", |_vm, counter, _value| {
            let value = counter.decrement();
            Ok(Value::Integer(value))
        });

        methods.add_method_mut("reset", |_vm, counter, _value| {
            counter.set_count(0);
            Ok(Value::Nil)
        });

        methods.add_meta_method("__tostring", |_vm, counter, _value| {
            Ok(Value::String(Box::new(format!(
                "Counter({})",
                counter.get_count()
            ))))
        });

        methods.add_meta_method("__add", |_vm, counter, value| {
            if let Value::Integer(n) = value {
                Ok(Value::Integer(counter.get_count() + n))
            } else {
                Err(LuaError::ExpOpValueWithValue(
                    crate::error::ValueTypes::UserData,
                    crate::userdata::MetaMethod::Add,
                    value.to_error(),
                ))
            }
        });
    }

    fn add_fields<'v, F: UserDataFields<'v, Self>>(fields: &mut F) {
        fields.add_field_method_get("count", |_vm, counter| {
            Ok(Value::Integer(counter.get_count()))
        });

        fields.add_field_method_set("count", |_vm, counter, value| {
            if let Value::Integer(n) = value {
                counter.set_count(n);
                Ok(Value::Nil)
            } else {
                Err(LuaError::ExpOpValueWithValue(
                    crate::error::ValueTypes::UserData,
                    crate::userdata::MetaMethod::NewIndex,
                    value.to_error(),
                ))
            }
        });
    }
}

pub fn make_userdata<'lua>(_: &mut VM, mc: &Mutation<'lua>, args: Vec<Value<'lua>>) -> Value<'lua> {
    // Register the Counter type with the VM
    vm.register_userdata_type::<Counter>();

    // Create a Counter instance
    let counter = Counter::new();

    vm.create_userdata(mc, counter)

    // Now in Lua code, you can use:
    // counter:increment()
    // counter:reset()
    // print(counter.count)
    // counter.count = 10
    // print(counter + 5)
}
