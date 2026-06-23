use silt_lua::{simple, test_int, valeq, ExVal};

// `#` returns an integer length. NOTE: silt currently returns the hashmap entry count rather
// than a Lua "border" for tables with holes (documented deviation, PLAN.md §4) — the cases
// here are hole-free so they match standard Lua.
test_int!(empty_table_len, "local t = {} return #t", 0);
test_int!(array_table_len, "local t = {1, 2, 3, 4, 5} return #t", 5);

#[test]
fn array_indexing() {
    valeq!("local t = {10, 20, 30} return t[2]", ExVal::Integer(20));
}

#[test]
fn bracket_assignment() {
    valeq!(
        "local t = {} t[1] = 42 t[2] = 84 return t[1] + t[2]",
        ExVal::Integer(126)
    );
}

#[test]
fn string_key_brackets() {
    valeq!(
        r#"local t = {} t["name"] = "John" return t["name"]"#,
        ExVal::String("John".to_string())
    );
}

#[test]
fn dot_notation() {
    valeq!("local t = {} t.x = 10 t.y = 20 return t.x + t.y", ExVal::Integer(30));
}

#[test]
fn constructor_with_string_keys() {
    valeq!("local t = {a = 1, b = 2, c = 3} return t.a + t.b + t.c", ExVal::Integer(6));
}

#[test]
fn mixed_constructor() {
    valeq!(
        "local t = {10, 20, x = 30, 40} return t[1] + t[2] + t.x + t[3]",
        ExVal::Integer(100)
    );
}

#[test]
fn negative_index() {
    valeq!("local t = {} t[-1] = 5 return t[-1]", ExVal::Integer(5));
}

// =====================================================================================
// BROKEN — see PLAN.md
// =====================================================================================

#[test]
#[ignore = "PLAN.md §2.10 — chained field read t.a.b (depth >= 2) resolves t.a to nil"]
fn nested_field_read() {
    valeq!("local t = {a = {b = 7}} return t.a.b", ExVal::Integer(7));
}

#[test]
#[ignore = "PLAN.md §2.10 — nested table literal not stored as a subtable"]
fn nested_constructor() {
    valeq!("local t = {inner = {value = 42}} return t.inner.value", ExVal::Integer(42));
}

#[test]
#[ignore = "PLAN.md §2.10 — chained field write t.a.b = v errors on the intermediate"]
fn nested_field_write() {
    valeq!("local t = {a = {}} t.a.b = 7 return t.a.b", ExVal::Integer(7));
}

#[test]
#[ignore = "PLAN.md §2.7/§3 — generic for + pairs not implemented"]
fn iteration_with_pairs() {
    valeq!(
        r#"
        local t = {a = 1, b = 2, c = 3}
        local sum = 0
        for k, v in pairs(t) do sum = sum + v end
        return sum
    "#,
        ExVal::Integer(6)
    );
}
