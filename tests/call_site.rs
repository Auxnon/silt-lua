//! `VM::call_site`: a native learns which chunk, function and source span called it.

use silt_lua::gc_arena::Mutation;
use silt_lua::{Compiler, ExVal, Lua, LuaError, Value, VM};

/// `here()` → "chunk|function|line:col-end_line:end_col", or "none" without a caller.
fn here<'l>(vm: &mut VM<'l>, mc: &Mutation<'l>, _: Vec<Value<'l>>) -> Result<Value<'l>, LuaError> {
    let s = match vm.call_site() {
        Some(c) => format!(
            "{}|{}|{}:{}-{}:{}",
            c.chunk.as_deref().unwrap_or("?"),
            c.function.as_deref().unwrap_or("-"),
            c.line,
            c.column,
            c.end_line,
            c.end_column
        ),
        None => "none".to_string(),
    };
    Ok(vm.new_string(mc, s))
}

fn lua() -> Lua {
    let mut lua = Lua::new_with_standard();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "here", here);
    });
    lua
}

fn run(lua: &mut Lua, name: Option<&str>, src: &str) -> ExVal {
    lua.run(name, src, &mut Compiler::new()).map_err(|e| e.to_string()).unwrap()
}

fn s(v: &str) -> ExVal {
    ExVal::String(v.to_string())
}

#[test]
fn top_level_call() {
    let mut lua = lua();
    assert_eq!(run(&mut lua, Some("main.lua"), "\n  return here()"), s("main.lua|-|2:10-2:15"));
}

#[test]
fn inside_a_function_and_across_lines() {
    let mut lua = lua();
    let src = "function spawn(x)\n  return here(\n    x\n  )\nend\nreturn spawn(1)";
    assert_eq!(run(&mut lua, Some("ents.lua"), src), s("ents.lua|spawn|2:10-4:3"));
}

#[test]
fn a_function_reports_the_chunk_it_was_written_in() {
    let mut lua = lua();
    run(&mut lua, Some("lib.lua"), "function where() return here() end");
    assert_eq!(run(&mut lua, Some("main.lua"), "return where()"), s("lib.lua|where|1:25-1:30"));
}

#[test]
fn natives_calling_back_into_script_keep_their_own_caller() {
    let mut lua = lua();
    // pcall is a native; the inner here() sees its own caller, and the outer call
    // site is restored once pcall returns
    let src = "local function f() return here() end\nlocal ok, inner = pcall(f)\nreturn inner .. ' ' .. here()";
    assert_eq!(run(&mut lua, None, src), s("?|f|1:27-1:32 ?|-|3:24-3:29"));
}

#[test]
fn no_call_site_outside_a_script_call() {
    let mut lua = lua();
    let out = lua.enter(|vm, _| vm.call_site().is_none());
    assert!(out);
}
