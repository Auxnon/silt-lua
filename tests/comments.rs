use silt_lua::{simple, valeq, ExVal};

// Regression: comments only worked at statement level. Inside a table constructor or
// mid-expression the `Token::Comment` was left in the stream and tripped
// "Invalid token placement". The parser's token accessors now skip comments, and the
// lexer learned `--[[ … ]]` block comments (so a comment can be truly *inline*, with
// code after it on the same line).

// --- line comments inside table constructors -------------------------------

#[test]
fn line_comment_between_table_fields() {
    valeq!(
        "local a = {1, 2, -- inline\n3} return a[3]",
        ExVal::Integer(3)
    );
}

#[test]
fn line_comment_as_table_header() {
    valeq!("local a = { -- header\n1,2,3} return a[1]", ExVal::Integer(1));
}

#[test]
fn line_comment_before_closing_brace() {
    valeq!("local a = {1,2,3 -- trailing\n} return #a", ExVal::Integer(3));
}

#[test]
fn line_comment_per_field() {
    valeq!(
        "local a = {\n1, -- one\n2, -- two\n3\n} return a[2]",
        ExVal::Integer(2)
    );
}

// --- block comments: inline, with code after them --------------------------

#[test]
fn block_comment_inline_in_table() {
    valeq!("local a = {1, --[[mid]] 2, 3} return a[2]", ExVal::Integer(2));
    valeq!(
        "local a = {--[[a]] 1, --[[b]] 2 --[[c]]} return #a",
        ExVal::Integer(2)
    );
}

#[test]
fn block_comment_mid_expression() {
    valeq!("return 1 + --[[x]] 2", ExVal::Integer(3));
    valeq!("for i=1, --[[n]] 3 do end return 0", ExVal::Integer(0));
}

#[test]
fn block_comment_multiline() {
    valeq!(
        "local a = --[[ this\nspans\nlines ]] 42 return a",
        ExVal::Integer(42)
    );
}

// --- edge cases ------------------------------------------------------------

#[test]
fn dash_bracket_is_not_a_block_comment() {
    // `--[` without a second `[` is an ordinary line comment.
    valeq!("local a = 5 --[ not a block\nreturn a", ExVal::Integer(5));
}

#[test]
fn plain_line_comment_still_works() {
    valeq!("local a = 5 -- normal\nreturn a", ExVal::Integer(5));
}
