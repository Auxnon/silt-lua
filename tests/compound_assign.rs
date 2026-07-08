// Luau-style compound assignment (`+= -= *= /= //= %= ^= ..=`), gated behind the
// `compound-assignment` cargo feature (on by default via the `silt` set) and the
// `LanguageFlags::compound_assignment` runtime flag. `x op= e` desugars to
// `x = x op e`, evaluating the target once.
#![cfg(feature = "compound-assignment")]
use silt_lua::{simple, valeq, ExVal};

#[test]
fn local_variable_ops() {
    valeq!("local x = 5 x += 3 return x", ExVal::Integer(8));
    valeq!("local x = 10 x -= 4 return x", ExVal::Integer(6));
    valeq!("local x = 3 x *= 4 return x", ExVal::Integer(12));
    valeq!("local x = 20 x /= 4 return x", ExVal::Number(5.0));
    valeq!("local x = 17 x %= 5 return x", ExVal::Integer(2));
    valeq!("local x = 2 x ^= 10 return x", ExVal::Number(1024.0));
    valeq!("local x = 17 x //= 5 return x", ExVal::Integer(3));
}

#[test]
fn string_concat_assign() {
    valeq!(
        r#"local s = "a" s ..= "b" s ..= "c" return s"#,
        ExVal::String("abc".to_string())
    );
}

#[test]
fn global_variable() {
    valeq!("g = 5 g += 2 return g", ExVal::Integer(7));
}

#[test]
fn table_field_and_index() {
    valeq!("local t = {x = 10} t.x += 5 return t.x", ExVal::Integer(15));
    valeq!("local t = {x = 10} t.x *= 3 return t.x", ExVal::Integer(30));
    valeq!("local t = {} t[1] = 4 t[1] += 6 return t[1]", ExVal::Integer(10));
    // chained receiver, target resolved once
    valeq!("local a = {b = {c = 1}} a.b.c += 41 return a.b.c", ExVal::Integer(42));
}

#[test]
fn upvalue_compound() {
    // compound assignment through a captured upvalue
    valeq!(
        r#"
        local function mk()
            local c = 0
            return function() c += 1 return c end
        end
        local f = mk()
        f() f()
        return f()
    "#,
        ExVal::Integer(3)
    );
}

#[test]
fn method_self_field() {
    valeq!(
        r#"
        local t = { n = 0 }
        function t:inc(by) self.n += by return self.n end
        t:inc(5)
        return t:inc(7)
    "#,
        ExVal::Integer(12)
    );
}

#[test]
fn loop_accumulator() {
    valeq!("local s = 0 for i = 1, 5 do s += i end return s", ExVal::Integer(15));
}

#[test]
fn index_evaluated_once() {
    // t[k] += v must read and write the same slot; here k is a plain key but the
    // DUP_N path means the key expression result is reused, not recomputed.
    valeq!(
        "local t = {} t[2] = 100 local k = 2 t[k] += 8 return t[2]",
        ExVal::Integer(108)
    );
}
