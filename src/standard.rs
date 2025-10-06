use gc_arena::Mutation;

use crate::{
    prelude::VM,
    userdata::{InnerResult, TestEnt},
    value::{Value, FromLuaMulti},
};

pub fn clock<'lua>(_: &mut VM<'lua>, _: &Mutation<'lua>, _: (f64,)) -> InnerResult<'lua> {
    Ok(Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    ))
}

pub fn print<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    println!("we called print");
    let s = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\t");
    println!("> {}", s);

    #[cfg(target_arch = "wasm32")]
    crate::jprintln(s.as_str());

    Ok(Value::Nil)
}

pub fn setmetatable<'lua>(
    _: &mut VM,
    mc: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    // println!("we caled setmetatable");
    // let t=args.
    // let metatable = args[1].clone();
    match &args[0] {
        Value::Table(t) => t.borrow_mut(mc).set_metatable(args[1].clone()),
        Value::String(_s) => {}
        _ => {
            println!("cant set metatable on this non table"); // TODO
        }
    }
    Ok(Value::Nil)
}

pub fn getmetatable<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    Ok(if let Value::Table(t) = args[0] {
        t.borrow().get_metatable()
    } else {
        Value::Nil
    })
}

pub fn select<'lua>(_: &mut VM, _: &Mutation<'lua>, _args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    // Value::Nil
    todo!()
}

pub fn test_ent<'lua>(
    vm: &mut VM<'lua>,
    mc: &Mutation<'lua>,
    _args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let e = TestEnt::new(4., 5., 6.);
    Ok(vm.create_userdata(mc, e))
}

// pub fn ud_trap<'lua>(vm: &mut VM<'lua>, mc: &Mutation<'lua>, args: Vec<Value<'lua>>) -> Value<'lua> {
//     // args.concat()
//
//     let ud=vm.yank(1);
// }
