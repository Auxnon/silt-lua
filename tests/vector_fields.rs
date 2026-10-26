//! Vector components read through an index chain (`t.n.x`). The compiler fuses
//! `a.b.c` into one `TABLE_GET { depth: 2 }`, so a vector can turn up as a link
//! of the walk as well as its receiver. Vectors are read-only values, so setting
//! a component is an error with its own message.
#![cfg(feature = "vector")]

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut compiler = Compiler::new();
    lua.run(None, src, &mut compiler).map_err(|e| format!("{:?}", e))
}

fn num(src: &str) -> f64 {
    match run(src) {
        Ok(ExVal::Number(n)) => n,
        other => panic!("{src}: {other:?}"),
    }
}

fn err(src: &str) -> String {
    match run(src) {
        Err(e) => e,
        Ok(v) => panic!("{src}: expected an error, got {v:?}"),
    }
}

#[test]
fn components_through_a_chain() {
    assert_eq!(num("local t = { n = vec3(1, 2, 3) } return t.n.x"), 1.0);
    assert_eq!(num("local t = { vec3(1, 2, 3) } return t[1].y"), 2.0);
    assert_eq!(num("local t = { a = { n = vec3(1, 2, 3) } } return t.a.n.z"), 3.0);
    assert_eq!(num("local t = { list = { vec2(1, 2), vec2(3, 4) } } return t.list[2].y"), 4.0);
    assert_eq!(num("local t = { n = vec2(5, 6) } return t.n.y"), 6.0);
    assert_eq!(num("local t = { n = vec4(1, 2, 3, 4) } return t.n.w"), 4.0);
    assert_eq!(num("g = { n = vec3(7, 8, 9) } return g.n.y"), 8.0);
}

#[test]
fn chains_in_functions_and_loops() {
    assert_eq!(
        num("local t = { n = vec3(1, 2, 3) } local function f() return t.n.y end return f()"),
        2.0
    );
    assert_eq!(
        num("local t = { n = vec3(1, 2, 3) } local s = 0 for i = 1, 10 do s = s + t.n.z end return s"),
        30.0
    );
}

#[test]
fn locals_after_a_chain_keep_their_slots() {
    // a chain exit must leave the stack exactly as a table exit does
    assert_eq!(
        num(
            "local t = { n = vec3(1, 2, 3), m = { k = 10 } }
             local a = t.n.x
             local b = t.m.k
             local c = t.n.z
             return a + b * 100 + c * 10000"
        ),
        1.0 + 1000.0 + 30000.0
    );
}

#[test]
fn missing_and_over_deep_fields_are_errors() {
    assert!(err("local t = { n = vec3(1, 2, 3) } return t.n.q").contains("'q' is not a field of vec3"));
    assert!(err("local v = vec2(1, 2) return v.z").contains("'z' is not a field of vec2"));
    // `v.x.y` used to read `v.y` silently
    assert!(err("local v = vec3(1, 2, 3) return v.x.y").contains("cannot index 'x' of vec3"));
    assert!(err("local t = { n = vec3(1, 2, 3) } return t.n.x.y").contains("cannot index 'x' of vec3"));
}

#[test]
fn components_are_read_only() {
    assert!(err("local v = vec3(1, 2, 3) v.x = 9").contains("vec3 fields are read-only"));
    assert!(err("local t = { n = vec3(1, 2, 3) } t.n.x = 9").contains("vec3 fields are read-only"));
}
