use crate::{silt::SiltLua, value::Value};

pub fn clock(_: &mut SiltLua, _: Vec<Value>) -> Value {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print(_: &mut SiltLua, args: Vec<Value>) -> Value {
    for arg in args {
        print!("> {}", arg);
    }
    println!();
    Value::Nil
}
