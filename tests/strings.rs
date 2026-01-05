mod shared;
use shared::*;

test_string!(simple_string, "return 'hello'", "hello");
test_string!(double_quote_string, "return \"world\"", "world");
test_string!(empty_string, "return ''", "");

test_string!(
    string_concatenation,
    "return 'hello' .. ' ' .. 'world'",
    "hello world"
);
test_string!(
    string_number_concat,
    "return 'number: ' .. 42",
    "number: 42"
);

#[test]
fn string_length() {
    valeq!("return #'hello'", ExVal::Number(5.0));
    valeq!("return #''", ExVal::Number(0.0));
    valeq!("return #'a'", ExVal::Number(1.0));
}

#[test]
fn string_comparison() {
    valeq!("return 'abc' == 'abc'", ExVal::Boolean(true));
    valeq!("return 'abc' == 'def'", ExVal::Boolean(false));
    valeq!("return 'abc' ~= 'def'", ExVal::Boolean(true));
    valeq!("return 'abc' < 'def'", ExVal::Boolean(true));
    valeq!("return 'def' > 'abc'", ExVal::Boolean(true));
}

#[test]
fn multiline_string() {
    let expected = "This is a\nmultiline string\nwith several lines";
    valeq!(
        r#"
        local s = [[This is a
multiline string
with several lines]]
        return s
    "#,
        ExVal::String(expected.to_string())
    );
}

#[test]
fn string_escape_sequences() {
    valeq!(
        r#"return 'hello\nworld'"#,
        ExVal::String("hello\nworld".to_string())
    );
    valeq!(
        r#"return 'tab\there'"#,
        ExVal::String("tab\there".to_string())
    );
    valeq!(
        r#"return 'quote: \"here\"'"#,
        ExVal::String("quote: \"here\"".to_string())
    );
}

#[test]
fn string_concatenation_chain() {
    let source = r#"
        local a = 'one'
        local b = 'two'
        local c = 'three'
        return a .. b .. c
    "#;
    valeq!(source, ExVal::String("onetwothree".to_string()));
}

#[test]
fn string_with_numbers() {
    valeq!("return '5' + '3'", ExVal::Number(8.0));
    valeq!("return '10' - '4'", ExVal::Number(6.0));
    valeq!("return '6' * '7'", ExVal::Number(42.0));
}
