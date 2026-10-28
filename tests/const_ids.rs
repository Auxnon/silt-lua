//! String constants carry a compiler-assigned id so equal names match by id
//! instead of by bytes. An id only ever proves equality: strings from another
//! compiler, or built at run time, must still find the same keys.

use silt_lua::{Compiler, ExVal, Lua};

fn run(lua: &mut Lua, compiler: &mut Compiler, src: &str) -> ExVal {
    lua.run(None, src, compiler).unwrap_or_else(|e| panic!("{src}: {e:?}"))
}

#[test]
fn globals_across_runs_of_one_compiler() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    run(&mut lua, &mut compiler, "function sq(n) return n * n end");
    assert_eq!(run(&mut lua, &mut compiler, "return sq(7)"), ExVal::Integer(49));
}

#[test]
fn globals_across_compilers() {
    let mut lua = Lua::new_with_standard();
    run(&mut lua, &mut Compiler::new(), "function sq(n) return n * n end total = 5");
    let mut other = Compiler::new();
    assert_eq!(run(&mut lua, &mut other, "return sq(3) + total"), ExVal::Integer(14));
}

#[test]
fn runtime_strings_find_constant_keys() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let src = r#"
        local t = { name = 1, other = 2 }
        local k = "na" .. "me"
        t["oth" .. "er"] = 20
        return t[k] + t.other
    "#;
    assert_eq!(run(&mut lua, &mut compiler, src), ExVal::Integer(21));
}

#[test]
fn different_names_stay_different() {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    let src = r#"
        local t = { a = 1, b = 2, ab = 3 }
        local function get(k) return t[k] end
        return get("a") * 100 + get("b") * 10 + get("ab")
    "#;
    assert_eq!(run(&mut lua, &mut compiler, src), ExVal::Integer(123));
    assert_eq!(run(&mut lua, &mut compiler, r#"return "a" == "b""#), ExVal::Bool(false));
    assert_eq!(run(&mut lua, &mut compiler, r#"return "a" == "a""#), ExVal::Bool(true));
}
