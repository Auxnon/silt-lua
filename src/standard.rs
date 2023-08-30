use crate::{jprintln, silt::SiltLua, value::Value};

pub fn clock(_: &mut SiltLua, _: Vec<Value>) -> Value {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print(_: &mut SiltLua, args: Vec<Value>) -> Value {
    let s = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\t");
    println!("> {}", s);
    jprintln(s.as_str());

    Value::Nil
}
