//! `Table` keeps the sequence `1..n` in a dense array part and everything else
//! in a hash part (PUC-Lua's layout; see SPEC-dual-array-table.md). These pin
//! the border (`#`), holes, out-of-order construction and iteration order.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}
fn int(src: &str, want: i64) {
    assert_eq!(run(src), Ok(ExVal::Integer(want)), "{src}");
}

#[test]
fn border_of_sequences() {
    int("return #{}", 0);
    int("return #{1, 2, 3}", 3);
    int("local t = {} for i = 1, 100 do t[i] = i end return #t", 100);
    // string keys do not count toward `#`
    int("local t = {a = 1, b = 2} return #t", 0);
    int("local t = {10, 20, x = 'y'} return #t", 2);
}

#[test]
fn removing_the_last_element_shrinks_the_border() {
    int("local t = {1, 2, 3} t[3] = nil return #t", 2);
    int("local t = {1, 2, 3} t[3] = nil t[2] = nil return #t", 1);
    int(
        "local t = {1, 2, 3} t[3] = nil t[2] = nil t[1] = nil return #t",
        0,
    );
}

#[test]
fn hole_in_the_middle_reads_nil_and_keeps_later_elements() {
    int(
        "local t = {1, 2, 3} t[2] = nil return (t[2] == nil and 1 or 0) * 10 + t[3]",
        13,
    );
    // refilling the hole works
    int(
        "local t = {1, 2, 3} t[2] = nil t[2] = 7 return t[1] + t[2] + t[3]",
        11,
    );
}

#[test]
fn out_of_order_build_collapses_into_a_dense_sequence() {
    int(
        "local t = {} t[3] = 30 t[1] = 10 t[2] = 20 return #t * 100 + t[1] + t[2] + t[3]",
        360,
    );
    // appending after the gap closes continues the sequence
    int(
        "local t = {} t[2] = 2 t[1] = 1 t[3] = 3 t[4] = 4 return #t",
        4,
    );
}

#[test]
fn sparse_integer_keys_stay_out_of_the_border() {
    int("local t = {} t[1000000] = 1 return #t", 0);
    int("local t = {} t[1000000] = 1 return t[1000000]", 1);
    int("local t = {1, 2} t[10] = 10 return #t", 2);
}

#[test]
fn constructor_nil_keeps_its_position() {
    int("local t = {1, nil, 3} return t[3]", 3);
    int("local t = {1, nil, 3} return (t[2] == nil) and 1 or 0", 1);
}

#[test]
fn append_at_border_plus_one() {
    int(
        "local t = {1, 2} t[#t + 1] = 3 t[#t + 1] = 4 return #t * 10 + t[4]",
        44,
    );
}

#[test]
fn table_insert_and_remove_shift_within_the_array() {
    int("local t = {1, 2, 3} table.insert(t, 2, 99) return t[1] * 1000 + t[2] * 100 + t[3] * 10 + t[4]", 1000 + 9900 + 20 + 3);
    int(
        "local t = {1, 2, 3} table.insert(t, 4) return #t * 10 + t[4]",
        44,
    );
    int(
        "local t = {1, 2, 3} local r = table.remove(t, 1) return r * 100 + #t * 10 + t[1]",
        122,
    );
    int(
        "local t = {1, 2, 3} local r = table.remove(t) return r * 10 + #t",
        32,
    );
}

#[test]
fn pairs_visits_array_part_in_order_then_hash_part() {
    // The array part comes first and in index order; the string key last.
    let src = "
        local t = {10, 20, 30, x = 'X'}
        local s = ''
        for k, v in pairs(t) do s = s .. tostring(k) .. '=' .. tostring(v) .. ';' end
        return s";
    assert_eq!(
        run(src),
        Ok(ExVal::String("1=10;2=20;3=30;x=X;".to_string()))
    );
}

#[test]
fn pairs_skips_holes_and_resumes_across_the_boundary() {
    let src = "
        local t = {1, 2, 3, 4, y = 'Y'}
        t[2] = nil
        local n, sum = 0, 0
        for k, v in pairs(t) do
            n = n + 1
            if type(v) == 'number' then sum = sum + v end
        end
        return n * 100 + sum";
    assert_eq!(run(src), Ok(ExVal::Integer(4 * 100 + 1 + 3 + 4)));
}

#[test]
fn ipairs_stops_at_the_first_hole() {
    int("local t = {1, 2, 3, 4} t[3] = nil local n = 0 for _ in ipairs(t) do n = n + 1 end return n", 2);
}

#[test]
fn assigning_nil_to_a_string_key_removes_it() {
    let src = "
        local t = {a = 1, b = 2}
        t.a = nil
        local n = 0
        for _ in pairs(t) do n = n + 1 end
        return n";
    assert_eq!(run(src), Ok(ExVal::Integer(1)));
}
