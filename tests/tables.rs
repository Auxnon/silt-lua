mod shared;
use shared::*;

#[test]
fn empty_table() {
    let source = r#"
        local t = {}
        return #t
    "#;
    valeq!(source, ExVal::Number(0.0));
}

#[test]
fn table_with_values() {
    let source = r#"
        local t = {1, 2, 3, 4, 5}
        return #t
    "#;
    valeq!(source, ExVal::Number(5.0));
}

#[test]
fn table_indexing() {
    let source = r#"
        local t = {10, 20, 30}
        return t[2]
    "#;
    valeq!(source, ExVal::Number(20.0));
}

#[test]
fn table_assignment() {
    let source = r#"
        local t = {}
        t[1] = 42
        t[2] = 84
        return t[1] + t[2]
    "#;
    valeq!(source, ExVal::Number(126.0));
}

#[test]
fn table_string_keys() {
    let source = r#"
        local t = {}
        t["name"] = "John"
        t["age"] = 30
        return t["name"]
    "#;
    valeq!(source, ExVal::String("John".to_string()));
}

#[test]
fn table_dot_notation() {
    let source = r#"
        local t = {}
        t.x = 10
        t.y = 20
        return t.x + t.y
    "#;
    valeq!(source, ExVal::Number(30.0));
}

#[test]
fn nested_tables() {
    let source = r#"
        local t = {
            inner = {
                value = 42
            }
        }
        return t.inner.value
    "#;
    valeq!(source, ExVal::Number(42.0));
}

#[test]
fn table_constructor_with_keys() {
    let source = r#"
        local t = {
            a = 1,
            b = 2,
            c = 3
        }
        return t.a + t.b + t.c
    "#;
    valeq!(source, ExVal::Number(6.0));
}

#[test]
fn mixed_table_constructor() {
    let source = r#"
        local t = {
            10, 20,
            x = 30,
            40
        }
        return t[1] + t[2] + t.x + t[3]
    "#;
    valeq!(source, ExVal::Number(100.0));
}

#[test]
fn table_iteration_with_pairs() {
    let source = r#"
        local t = {a = 1, b = 2, c = 3}
        local sum = 0
        for k, v in pairs(t) do
            sum = sum + v
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(6.0));
}
