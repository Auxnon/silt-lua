use crate::{environment::Environment, value::Value, vm::VM};

pub fn clock(_: &mut VM, _: Vec<Value>) -> Value {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print(_: &mut VM, args: Vec<Value>) -> Value {
    for arg in args {
        print!("> {}", arg);
    }
    println!();
    Value::Nil
}
