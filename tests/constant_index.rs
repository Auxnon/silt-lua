//! Regression: constant-table indices were u8, and the compiler narrowed with a
//! silent `as u8`, so a chunk with more than 256 constants read the *wrong*
//! constants — `T[130] = "s130"` stored some other literal, and a wrapped
//! identifier made GET_GLOBAL fetch an unrelated global. Indices are now u16, and
//! overflowing that is a compile error instead of a wrap.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}

#[test]
fn more_than_256_constants_resolve_correctly() {
    // Two constants per line (the index and the string): ~400 in one chunk.
    let mut src = String::from("T = {}\n");
    for i in 1..=200 {
        src.push_str(&format!("T[{i}] = \"s{i}\"\n"));
    }
    src.push_str("return T[1] .. \" \" .. T[130] .. \" \" .. T[200]");
    assert_eq!(run(&src), Ok(ExVal::String("s1 s130 s200".to_string())));
}

#[test]
fn globals_past_256_constants_resolve_correctly() {
    // Identifiers are constants too: a wrapped one made GET_GLOBAL read another name.
    let mut src = String::new();
    for i in 0..300 {
        src.push_str(&format!("G{i} = {i}\n"));
    }
    src.push_str("return G0 + G255 + G256 + G299");
    assert_eq!(run(&src), Ok(ExVal::Integer(0 + 255 + 256 + 299)));
}

#[test]
fn closures_past_256_constants_resolve_correctly() {
    // Function prototypes live in the constant table (CLOSURE).
    let mut src = String::new();
    for i in 0..300 {
        src.push_str(&format!("function f{i}() return {i} end\n"));
    }
    src.push_str("return f0() + f200() + f299()");
    assert_eq!(run(&src), Ok(ExVal::Integer(0 + 200 + 299)));
}

// Each `x = N` adds one number constant (literals aren't deduplicated).
fn numeric_constants(n: usize) -> String {
    let mut src = String::from("local x = 0\n");
    for i in 1..n {
        src.push_str(&format!("x = {i}\n"));
    }
    src.push_str("return x");
    src
}

#[test]
fn near_the_limit_still_runs() {
    assert_eq!(run(&numeric_constants(65000)), Ok(ExVal::Integer(64999)));
}

#[test]
fn overflowing_the_constant_table_is_a_compile_error() {
    let err = run(&numeric_constants(65537)).unwrap_err();
    assert!(
        err.contains("Too many constants"),
        "unexpected error: {err}"
    );
}
