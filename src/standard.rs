use crate::{environment::Environment, value::Value};

pub fn clock(_: &mut Environment, _: Vec<Value>) -> Value {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}
