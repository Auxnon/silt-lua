use crate::{lua::Lua, value::Value};

pub fn clock<'lua>(_: &mut Lua, _: Vec<Value>) -> Value<'lua> {
    Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    )
}

pub fn print<'lua>(_: &mut Lua, args: Vec<Value>) -> Value<'lua> {
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
