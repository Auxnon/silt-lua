use silt_lua::{simple, test_int, test_string, valeq, ExVal};

test_string!(simple_string, "return 'hello'", "hello");
test_string!(double_quote_string, "return \"world\"", "world");
test_string!(empty_string, "return ''", "");

test_string!(
    string_concatenation,
    "return 'hello' .. ' ' .. 'world'",
    "hello world"
);
test_string!(string_number_concat, "return 'number: ' .. 42", "number: 42");

#[test]
fn string_concat_chain() {
    valeq!(
        r#"
        local a = 'one'
        local b = 'two'
        local c = 'three'
        return a .. b .. c
    "#,
        ExVal::String("onetwothree".to_string())
    );
}

// `#` on a string yields an integer length.
test_int!(string_length_hello, "return #'hello'", 5);
test_int!(string_length_empty, "return #''", 0);
test_int!(string_length_one, "return #'a'", 1);

#[test]
fn string_equality() {
    valeq!("return 'abc' == 'abc'", ExVal::Bool(true));
    valeq!("return 'abc' == 'def'", ExVal::Bool(false));
    valeq!("return 'abc' ~= 'def'", ExVal::Bool(true));
}

#[test]
fn multiline_string_no_escape_decoding() {
    // Long-bracket strings are raw: no escape processing, literal newlines preserved.
    let expected = "This is a\nmultiline string\nwith several lines";
    valeq!(
        "return [[This is a\nmultiline string\nwith several lines]]",
        ExVal::String(expected.to_string())
    );
}

#[test]
fn string_arithmetic_coercion() {
    // numeric strings coerce in arithmetic and the result is an integer
    valeq!("return '5' + '3'", ExVal::Integer(8));
    valeq!("return '10' - '4'", ExVal::Integer(6));
    valeq!("return '6' * '7'", ExVal::Integer(42));
}

// =====================================================================================
// BROKEN — see PLAN.md
// =====================================================================================

#[test]
fn string_comparison() {
    valeq!("return 'abc' < 'abd'", ExVal::Bool(true));
    valeq!("return 'abc' <= 'abc'", ExVal::Bool(true));
    valeq!("return 'def' > 'abc'", ExVal::Bool(true));
    valeq!("return 'b' >= 'a'", ExVal::Bool(true));
    valeq!("return 'abc' < 'ab'", ExVal::Bool(false)); // prefix is greater
    valeq!("return 'Z' < 'a'", ExVal::Bool(true)); // byte order: upper < lower
}

#[test]
fn string_escape_sequences() {
    valeq!(r#"return 'hello\nworld'"#, ExVal::String("hello\nworld".to_string()));
    valeq!(r#"return 'tab\there'"#, ExVal::String("tab\there".to_string()));
    valeq!(r#"return 'quote: \"x\"'"#, ExVal::String("quote: \"x\"".to_string()));
    valeq!(r#"return 'back\\slash'"#, ExVal::String("back\\slash".to_string()));
    valeq!(r#"return 'a\rb\0c'"#, ExVal::String("a\rb\0c".to_string()));
    // escaped delimiter does not terminate the literal
    valeq!(r#"return 'it\'s'"#, ExVal::String("it's".to_string()));
    // #-length sees decoded bytes, not the backslash
    valeq!(r#"return #'a\nb'"#, ExVal::Integer(3));
    // long-bracket strings do NOT decode escapes
    valeq!("return [[a\\nb]]", ExVal::String("a\\nb".to_string()));
}

#[test]
#[ignore = "PLAN.md §3 — string library + string metatable not wired (s:method())"]
fn string_methods() {
    valeq!("return ('hi'):upper()", ExVal::String("HI".to_string()));
    valeq!("return string.sub('hello', 1, 3)", ExVal::String("hel".to_string()));
    valeq!("return string.rep('ab', 3)", ExVal::String("ababab".to_string()));
}
