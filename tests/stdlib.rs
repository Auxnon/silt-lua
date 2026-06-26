// Standard-library coverage. Only print/clock/setmetatable/getmetatable/table.insert/
// table.remove are registered today (src/standard.rs). Everything below is unimplemented —
// see PLAN.md §3. These encode the target behavior; remove #[ignore] as each lands.
use silt_lua::{simple, valeq, ExVal};

#[test]
fn base_type() {
    valeq!("return type(5)", ExVal::String("number".to_string()));
    valeq!("return type('x')", ExVal::String("string".to_string()));
    valeq!("return type(true)", ExVal::String("boolean".to_string()));
    valeq!("return type(nil)", ExVal::String("nil".to_string()));
    valeq!("return type({})", ExVal::String("table".to_string()));
    valeq!("return type(print)", ExVal::String("function".to_string()));
}

#[test]
fn base_tostring() {
    valeq!("return tostring(5)", ExVal::String("5".to_string()));
    valeq!("return tostring(true)", ExVal::String("true".to_string()));
    valeq!("return tostring(nil)", ExVal::String("nil".to_string()));
}

#[test]
fn base_tonumber() {
    valeq!("return tonumber('42')", ExVal::Integer(42));
    valeq!("return tonumber('3.5')", ExVal::Number(3.5));
    valeq!("return tonumber('ff', 16)", ExVal::Integer(255));
    valeq!("return tonumber('nan-sense')", ExVal::Nil);
}

#[test]
fn base_assert() {
    valeq!("assert(true) return 1", ExVal::Integer(1));
    valeq!("assert(1 == 1, 'should hold') return 2", ExVal::Integer(2));
}

#[test]
#[ignore = "PLAN.md §3 — `pcall`/`error` not implemented"]
fn base_pcall() {
    valeq!("local ok = pcall(function() return 1 end) return ok", ExVal::Bool(true));
    valeq!("local ok = pcall(function() error('boom') end) return ok", ExVal::Bool(false));
}

#[test]
#[ignore = "PLAN.md §3 — `select` is a todo!()"]
fn base_select() {
    valeq!("return select('#', 1, 2, 3)", ExVal::Integer(3));
    valeq!("return select(2, 'a', 'b', 'c')", ExVal::String("b".to_string()));
}

#[test]
fn math_library() {
    valeq!("return math.floor(3.7)", ExVal::Integer(3));
    valeq!("return math.ceil(3.2)", ExVal::Integer(4));
    valeq!("return math.abs(-5)", ExVal::Integer(5));
    valeq!("return math.max(1, 9, 4)", ExVal::Integer(9));
    valeq!("return math.min(1, 9, 4)", ExVal::Integer(1));
    valeq!("return math.sqrt(16)", ExVal::Number(4.0));
}

#[test]
fn string_library() {
    valeq!("return string.len('hello')", ExVal::Integer(5));
    valeq!("return string.sub('hello', 2, 4)", ExVal::String("ell".to_string()));
    valeq!("return string.upper('hi')", ExVal::String("HI".to_string()));
    valeq!("return string.lower('HI')", ExVal::String("hi".to_string()));
    valeq!("return string.rep('ab', 3)", ExVal::String("ababab".to_string()));
    valeq!("return string.format('%d-%s', 5, 'x')", ExVal::String("5-x".to_string()));
}

#[test]
#[ignore = "PLAN.md §3 — `table.concat`/`table.unpack` not implemented"]
fn table_library() {
    valeq!("return table.concat({'a', 'b', 'c'}, ',')", ExVal::String("a,b,c".to_string()));
}
