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

// --- key hashing ------------------------------------------------------------
// Regression: `Hash for Value` hashed only the enum discriminant, so every key of a
// given type (all integers, all strings, …) collided into one bucket and `HashMap::get`
// degraded to an O(n) linear scan — every table access was O(table size). Now the
// payload is hashed. These assert correctness across key types and sizes; the payoff is
// performance (a 1000-entry table read went ~772ns → ~50ns, ~16x on array-heavy code).

#[test]
fn many_integer_keys_resolve() {
    valeq!(
        "local t = {} for i=1,1000 do t[i] = i*2 end return t[1] + t[500] + t[1000]",
        ExVal::Integer(3002) // 2 + 1000 + 2000
    );
}

#[test]
fn many_string_keys_resolve() {
    valeq!(
        r#"local t = {} for i=1,200 do t["k"..i] = i end return t.k1 + t.k100 + t.k200"#,
        ExVal::Integer(301)
    );
}

#[test]
fn assorted_key_types_resolve() {
    // negative, zero, and non-integer float keys all coexist
    valeq!("local t={} t[-5]=9 t[0]=8 t[1.5]=7 return t[-5]+t[0]+t[1.5]", ExVal::Integer(24));
    // bool keys
    valeq!("local t={[true]=1,[false]=2} return t[true]+t[false]", ExVal::Integer(3));
}

#[test]
fn signed_zero_is_one_key() {
    // 0.0 == -0.0 in PartialEq, so they must hash equal and address the same slot.
    valeq!("local t={} t[0.0]=1 t[-0.0]=2 return t[0.0]", ExVal::Integer(2));
    valeq!("local t={} t[0.0]=1 t[-0.0]=2 return t[-0.0]", ExVal::Integer(2));
}

// SILT-BUGS.md #2 & #3: table.insert/remove on a table grown by index assignment
// (`t[#t+1] = v`, how any line/file buffer is built) used to overwrite / leave a hole,
// because the shift loops were bounded by a `counter` that only the `{...}` constructor
// path maintained — `t[k] = v` never touched it. `border()` now derives the array length
// from actual contents, so insert/remove work regardless of how the table was built.

#[test]
fn insert_positional_on_loop_built_table() {
    valeq!(
        r#"
        local t = {}
        for i = 1, 19 do t[#t + 1] = "line" .. i end
        table.insert(t, 2, "INS")
        return #t
        "#,
        ExVal::Integer(20)
    );
    valeq!(
        r#"
        local t = {}
        for i = 1, 19 do t[#t + 1] = "line" .. i end
        table.insert(t, 2, "INS")
        return t[2] .. "/" .. t[3]
        "#,
        ExVal::String("INS/line2".to_string())
    );
}

#[test]
fn remove_positional_on_loop_built_table() {
    // remove(2) returns "line2", shifts the rest down, no nil hole
    valeq!(
        r#"
        local t = {}
        for i = 1, 19 do t[#t + 1] = "line" .. i end
        local r = table.remove(t, 2)
        return r .. "/" .. #t .. "/" .. t[2]
        "#,
        ExVal::String("line2/18/line3".to_string())
    );
}

#[test]
fn remove_last_on_loop_built_table() {
    valeq!(
        r#"
        local t = {}
        for i = 1, 5 do t[#t + 1] = i end
        local x = table.remove(t)
        return x * 10 + #t
        "#,
        ExVal::Integer(54) // removed 5, length now 4
    );
}

#[test]
fn border_matches_after_mixed_growth() {
    // grow by index, then append with table.insert, then check #t and ipairs agree
    valeq!(
        r#"
        local t = {}
        for i = 1, 3 do t[#t + 1] = i end
        table.insert(t, 4)
        table.insert(t, 1, 0)
        local sum = 0
        for _, v in ipairs(t) do sum = sum + v end
        return #t * 100 + sum
        "#,
        ExVal::Integer(510) // #t=5, sum=0+1+2+3+4=10
    );
}

// =====================================================================================
// BROKEN — see PLAN.md
// =====================================================================================

#[test]
fn nested_field_read() {
    valeq!("local t = {a = {b = 7}} return t.a.b", ExVal::Integer(7));
}

#[test]
fn nested_constructor() {
    valeq!("local t = {inner = {value = 42}} return t.inner.value", ExVal::Integer(42));
}

#[test]
fn nested_field_write() {
    valeq!("local t = {a = {}} t.a.b = 7 return t.a.b", ExVal::Integer(7));
}

#[test]
fn deep_chain_read_write() {
    // depth-3 navigation in both directions
    valeq!("local t = {a = {b = {c = 99}}} return t.a.b.c", ExVal::Integer(99));
    valeq!("local t = {a = {b = {}}} t.a.b.c = 5 return t.a.b.c", ExVal::Integer(5));
}

// Table constructor separators: `,` and `;` are interchangeable, and a trailing one is
// allowed. Regression: a trailing separator errored ("Invalid token placement: }").

#[test]
fn trailing_comma_in_constructor() {
    valeq!("local a = {1, 2, 3,} return a[3]", ExVal::Integer(3));
    valeq!("local a = {1, 2, 3, 4,} return #a", ExVal::Integer(4));
    valeq!("local a = {x = 1, y = 2,} return a.y", ExVal::Integer(2));
}

#[test]
fn trailing_comma_in_nested_constructor() {
    valeq!("local a = {{1, 2, 3}, {4, 5, 6},} return a[2][1]", ExVal::Integer(4));
}

#[test]
fn semicolon_separators() {
    valeq!("local a = {1; 2; 3} return a[2]", ExVal::Integer(2));
    // `;` and `,` may be mixed, including a trailing one.
    valeq!("local a = {1, 2; 3,} return #a", ExVal::Integer(3));
}

// Field/index access on a grouped or call-result expression — previously a parser
// gap ("Invalid token placement"); `.`/`[` are now Pratt infix operators.

#[test]
fn field_access_on_grouped_expression() {
    valeq!("local t = {x = 5} return (t).x", ExVal::Integer(5));
    valeq!(r#"local t = {x = 5} return (t)["x"]"#, ExVal::Integer(5));
}

#[test]
fn field_access_on_conditional_expression() {
    valeq!(
        "local t = {x = 1} local u = {x = 2} return (true and t or u).x",
        ExVal::Integer(1)
    );
}

#[test]
fn call_result_field_and_call() {
    // `f()` returns a table; index it, and also call a function field off a group.
    valeq!(
        "local function mk() return {v = 7} end return mk().v",
        ExVal::Integer(7)
    );
    valeq!(
        "local t = {f = function() return 9 end} return (t).f()",
        ExVal::Integer(9)
    );
}

#[test]
fn chained_access_after_group() {
    valeq!("local a = {b = {c = 42}} return (a).b.c", ExVal::Integer(42));
}

#[test]
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

// --- table.insert (append form) --------------------------------------------
// Regression: `Table::push` read `counter` without incrementing, so the first
// append landed at index 0 (Lua tables are 1-indexed) and later appends clobbered
// the last slot.

#[test]
fn insert_append_to_empty() {
    valeq!("local t = {} table.insert(t, 5) return t[1]", ExVal::Integer(5));
    valeq!("local t = {} table.insert(t, 5) return #t", ExVal::Integer(1));
}

#[test]
fn insert_append_multiple() {
    valeq!(
        "local t = {} table.insert(t, 5) table.insert(t, 6) return t[1] + t[2]",
        ExVal::Integer(11)
    );
}

#[test]
fn insert_append_to_existing() {
    valeq!("local t = {1, 2, 3} table.insert(t, 4) return t[4]", ExVal::Integer(4));
    valeq!("local t = {1, 2, 3} table.insert(t, 4) return #t", ExVal::Integer(4));
}

// --- table.insert (positional form, shifts elements up) --------------------
// Regression: the shift loop ran `counter..pos`, which is empty when `pos < counter`,
// so nothing shifted and the element already at `pos` was silently overwritten.

#[test]
fn insert_positional_shifts_up() {
    // {1,2} + insert(2,99) => {1,99,2}
    valeq!(
        "local t = {1, 2} table.insert(t, 2, 99) return t[1] * 100 + t[2] + t[3] * 1000",
        ExVal::Integer(2199) // 1*100 + 99 + 2*1000
    );
    valeq!("local t = {1, 2} table.insert(t, 2, 99) return #t", ExVal::Integer(3));
}

#[test]
fn insert_positional_at_front() {
    // {1,2,3} + insert(1,0) => {0,1,2,3}
    valeq!(
        "local t = {1,2,3} table.insert(t, 1, 0) return t[1]*1000 + t[2]*100 + t[3]*10 + t[4]",
        ExVal::Integer(123) // 0,1,2,3
    );
    valeq!("local t = {1,2,3} table.insert(t, 1, 0) return #t", ExVal::Integer(4));
}

// --- table.concat -----------------------------------------------------------

#[test]
fn concat_no_separator() {
    valeq!(
        r#"local t = {"a", "b", "c"} return table.concat(t)"#,
        ExVal::String("abc".to_string())
    );
}

#[test]
fn concat_with_separator() {
    valeq!(
        r#"local t = {"a", "b", "c"} return table.concat(t, ",")"#,
        ExVal::String("a,b,c".to_string())
    );
    valeq!(
        r#"local t = {1, 2, 3} return table.concat(t, "-")"#,
        ExVal::String("1-2-3".to_string())
    );
}

// --- integration: build with insert, then concat / iterate ------------------

#[test]
fn insert_then_concat() {
    valeq!(
        r#"
        local t = {}
        table.insert(t, "a")
        table.insert(t, "b")
        table.insert(t, "c")
        return table.concat(t, ",")
    "#,
        ExVal::String("a,b,c".to_string())
    );
}

#[test]
fn insert_then_ipairs() {
    valeq!(
        r#"
        local t = {}
        table.insert(t, 10)
        table.insert(t, 20)
        table.insert(t, 30)
        local s = 0
        for i, v in ipairs(t) do s = s + v end
        return s
    "#,
        ExVal::Integer(60)
    );
}

// --- table.remove ----------------------------------------------------------
// Regression: `table_remove` called `insert`/`push` (adding, not removing) — the
// default form grew the table and the positional form corrupted it / stack-overflowed.

#[test]
fn remove_last_returns_value_and_shrinks() {
    valeq!("local t = {1,2,3} return table.remove(t)", ExVal::Integer(3));
    valeq!("local t = {1,2,3} table.remove(t) return #t", ExVal::Integer(2));
    valeq!(
        "local t = {1,2,3} table.remove(t) return t[3] == nil",
        ExVal::Bool(true)
    );
}

#[test]
fn remove_positional_shifts_down() {
    // {1,2,3} remove(1) => returns 1, leaves {2,3}
    valeq!("local t = {1,2,3} return table.remove(t, 1)", ExVal::Integer(1));
    valeq!(
        "local t = {1,2,3} table.remove(t, 1) return #t * 100 + t[1] * 10 + t[2]",
        ExVal::Integer(223) // #t=2, t[1]=2, t[2]=3
    );
    // {10,20,30} remove(2) => leaves {10,30}
    valeq!(
        "local t = {10,20,30} table.remove(t, 2) return t[1] + t[2]",
        ExVal::Integer(40)
    );
}

#[test]
fn remove_on_empty_returns_nil() {
    valeq!("local t = {} return table.remove(t) == nil", ExVal::Bool(true));
}

#[test]
fn insert_remove_roundtrip() {
    valeq!(
        r#"
        local t = {}
        table.insert(t, "a")
        table.insert(t, "b")
        local x = table.remove(t)
        table.insert(t, "c")
        return t[1] .. t[2] .. x
    "#,
        ExVal::String("acb".to_string())
    );
}

// --- tostring identity for reference types ---------------------------------
// tostring already worked for scalars; tables/functions collapsed to bare
// "table"/"native_function" so distinct values compared equal. Now Lua-style.

#[test]
fn tostring_table_is_distinct_and_stable() {
    valeq!("local a={} local b={} return tostring(a) == tostring(b)", ExVal::Bool(false));
    valeq!("local a={} return tostring(a) == tostring(a)", ExVal::Bool(true));
    valeq!(
        r#"local a={} return string.sub(tostring(a), 1, 7)"#,
        ExVal::String("table: ".to_string())
    );
}

#[test]
fn tostring_function_prefix() {
    valeq!(
        r#"return string.sub(tostring(print), 1, 9)"#,
        ExVal::String("function:".to_string())
    );
}
