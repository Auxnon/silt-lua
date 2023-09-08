use crate::{lua::Lua, value::Value};

pub fn clock(_: &mut Lua, _: Vec<Value>) -> Value {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print(_: &mut Lua, args: Vec<Value>) -> Value {
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
