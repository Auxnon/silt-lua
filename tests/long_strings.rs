//! Long bracket strings and comments, any level, as in Lua 5.4 (values checked
//! against Lua 5.4.8).

use silt_lua::{simple, valeq, ExVal};

fn s(v: &str) -> ExVal {
    ExVal::String(v.to_string())
}

#[test]
fn levels_and_brackets_inside() {
    valeq!("return [[abc]]", s("abc"));
    valeq!("return [==[x]]y]=]z]==]", s("x]]y]=]z"));
    valeq!("return [[a]b]]", s("a]b"));
    valeq!("return [=[]]]=]", s("]]"));
}

#[test]
fn a_newline_after_the_opening_bracket_is_dropped() {
    valeq!("return [[\nfirst]]", s("first"));
    valeq!("return #[=[\n]]\n]=]", ExVal::Integer(3));
}

#[test]
fn block_comments_take_levels_too() {
    valeq!("--[==[ block\ncomment ]] still ]==] local e = 5 return e", ExVal::Integer(5));
    // `--[=` without a second `[` is a line comment
    valeq!("--[=x not a block\nreturn 7", ExVal::Integer(7));
}
