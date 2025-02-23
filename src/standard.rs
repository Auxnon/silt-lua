use gc_arena::Mutation;

use crate::{prelude::VM, value::Value};

pub fn clock<'lua>(_: &mut VM, _: &Mutation<'lua>, _: Vec<Value>) -> Value<'lua> {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> Value<'lua> {
    let s = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\t");
    println!("> {}", s);

    #[cfg(target_arch = "wasm32")]
    crate::jprintln(s.as_str());

    Value::Nil
}

pub fn setmetatable<'lua>(_: &mut VM, mc: &Mutation<'lua>, args: Vec<Value<'lua>>) -> Value<'lua> {
    // let t=args.
    let metatable = args[1].clone();
    if let Value::Table(t) = args[0] {
        t.borrow_mut(mc).set_metatable(args[1].clone());
    }
    Value::Nil
}

pub fn getmetatable<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> Value<'lua> {
    if let Value::Table(t) = args[0] {
        t.borrow().get_metatable()
    } else {
        Value::Nil
    }
}
