//! Static type checking (cargo feature `typing`). Run with:
//!   cargo test --features typing --test typing_checks
//!
//! Nonstrict (default): only annotated boundaries are checked. `--!strict`
//! adds operator checks and local inference. A type error fails compilation.
#![cfg(feature = "typing")]

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.run(None, src, &mut comp).map_err(|e| e.to_string())
}
fn ok(src: &str, want: ExVal) {
    assert_eq!(run(src), Ok(want), "{src}");
}
fn type_error(src: &str, needle: &str) {
    match run(src) {
        Err(e) => assert!(
            e.contains("TypeError") && e.contains(needle),
            "expected a TypeError containing {needle:?}, got: {e}\n{src}"
        ),
        Ok(v) => panic!("expected a type error containing {needle:?}, but it ran: {v:?}\n{src}"),
    }
}

// ----------------------------------------------------------- annotations

#[test]
fn annotated_local_mismatch() {
    type_error(
        "local n: number = 'x'",
        "Type 'string' could not be converted into 'number'",
    );
    type_error(
        "local s: string = 5",
        "Type 'number' could not be converted into 'string'",
    );
    type_error(
        "local b: boolean = nil",
        "Type 'nil' could not be converted into 'boolean'",
    );
}

#[test]
fn annotated_local_ok_and_gradual() {
    ok("local n: number = 5 return n", ExVal::Integer(5));
    ok("local a: any = 'x' a = 5 return a", ExVal::Integer(5));
    // unknown values never error
    ok(
        "local n: number = tonumber('7') or 0 return n",
        ExVal::Integer(7),
    );
    ok(
        "local n: number = unknown_global return 1",
        ExVal::Integer(1),
    );
}

#[test]
fn later_assignment_to_annotated_local() {
    type_error(
        "local n: number = 1 n = 'two'",
        "could not be converted into 'number'",
    );
    ok("local n: number = 1 n = 2 return n", ExVal::Integer(2));
}

#[test]
fn assignment_through_an_upvalue() {
    type_error(
        "local n: number = 1 local function f() n = 'x' end f()",
        "could not be converted into 'number'",
    );
}

#[test]
fn optional_types() {
    ok("local n: number? = nil n = 3 return n", ExVal::Integer(3));
    type_error(
        "local n: number? = 'x'",
        "could not be converted into 'number?'",
    );
    // an optional does not fit a plain slot
    type_error(
        "local a: number? = 1 local b: number = a",
        "could not be converted into 'number'",
    );
}

#[test]
fn union_types() {
    ok(
        "local v: number | string = 1 v = 'one' return v",
        ExVal::String("one".to_string()),
    );
    type_error(
        "local v: number | string = true",
        "could not be converted into 'number | string'",
    );
}

#[test]
fn unannotated_locals_stay_dynamic_in_nonstrict() {
    ok(
        "local x = 1 x = 'now a string' return x",
        ExVal::String("now a string".to_string()),
    );
    // string coercion in arithmetic is a runtime matter here
    ok("return '1' + 2", ExVal::Integer(3));
}

// ----------------------------------------------------------- functions

#[test]
fn typed_parameters_check_arguments() {
    type_error(
        "local function f(a: number) return a end return f('x')",
        "could not be converted into 'number' (argument 1)",
    );
    ok(
        "local function f(a: number) return a end return f(4)",
        ExVal::Integer(4),
    );
}

#[test]
fn return_type_checked() {
    type_error(
        "local function f(): number return 'x' end return f()",
        "(return value 1)",
    );
    ok(
        "local function f(): number return 9 end return f()",
        ExVal::Integer(9),
    );
    type_error(
        "local function f(): (number, string) return 1, 2 end return f()",
        "could not be converted into 'string' (return value 2)",
    );
}

#[test]
fn call_result_type_flows_into_annotations() {
    type_error(
        "local function f(): string return 's' end local n: number = f()",
        "Type 'string' could not be converted into 'number'",
    );
    ok(
        "local function f(): number return 2 end local n: number = f() return n",
        ExVal::Integer(2),
    );
}

#[test]
fn forward_reference_uses_prescanned_signature() {
    // `g` is called before its definition; the pre-scan knows its signature.
    type_error(
        "local function f() return g('x') end function g(n: number): number return n end return f()",
        "could not be converted into 'number' (argument 1)",
    );
    ok(
        "local function f() return g(2) end function g(n: number): number return n * 2 end return f()",
        ExVal::Integer(4),
    );
}

#[test]
fn recursive_local_function_checks_its_own_calls() {
    type_error(
        "local function fact(n: number): number if n <= 1 then return 1 end return fact('x') end return fact(3)",
        "(argument 1)",
    );
}

#[test]
fn function_typed_values() {
    type_error(
        "local f: (number) -> number = function(s: string) return 1 end",
        "could not be converted into '(number) -> number'",
    );
    ok(
        "local f: (number) -> number = function(n: number): number return n + 1 end return f(1)",
        ExVal::Integer(2),
    );
}

#[test]
fn builtin_signatures() {
    type_error(
        "local n: number = tostring(5)",
        "Type 'string' could not be converted into 'number'",
    );
    ok(
        "local s: string = type(5) return s",
        ExVal::String("number".to_string()),
    );
}

// -------------------------------------------------------------- tables

#[test]
fn table_shape_fields() {
    ok(
        "type P = {x: number, y: number} local p: P = {x = 1, y = 2} local n: number = p.x return n",
        ExVal::Integer(1),
    );
    type_error(
        "type P = {x: number, y: number} local p: P = {x = 1, y = 2} local s: string = p.x",
        "Type 'number' could not be converted into 'string'",
    );
    type_error(
        "type P = {x: number} local p: P = {x = 1} return p.z",
        "Key 'z' not found in table",
    );
}

#[test]
fn table_shape_structural_assignability() {
    ok(
        "type A = {x: number} type B = {x: number, y: string} local b: B = {x = 1, y = 's'} local a: A = b return a.x",
        ExVal::Integer(1),
    );
    type_error(
        "type A = {x: number, y: string} type B = {x: number} local b: B = {x = 1} local a: A = b",
        "could not be converted into",
    );
}

#[test]
fn array_shape_elements() {
    ok(
        "local xs: {number} = {1, 2, 3} local n: number? = xs[1] return n",
        ExVal::Integer(1),
    );
    type_error(
        "local xs: {number} = {1, 2, 3} local s: string = xs[1]",
        "could not be converted into 'string'",
    );
}

#[test]
fn type_aliases_resolve_and_may_be_used_before_definition() {
    ok(
        "local n: Num = 3 type Num = number return n",
        ExVal::Integer(3),
    );
    type_error(
        "local n: Num = 'x' type Num = number",
        "could not be converted into 'number'",
    );
}

// ----------------------------------------------------------- narrowing

#[test]
fn narrowing_optional_in_if() {
    ok(
        "local function f(n: number?): number if n ~= nil then return n end return 0 end return f(5)",
        ExVal::Integer(5),
    );
    ok(
        "local function f(n: number?): number if n then return n end return 0 end return f(nil)",
        ExVal::Integer(0),
    );
    // without the guard the optional does not fit the return type
    type_error(
        "local function f(n: number?): number return n end",
        "could not be converted into 'number'",
    );
}

#[test]
fn narrowing_with_type_function() {
    ok(
        "local function f(v: number | string): number if type(v) == 'number' then return v end return 0 end return f(7)",
        ExVal::Integer(7),
    );
    type_error(
        "local function f(v: number | string): number if type(v) == 'string' then return v end return 0 end",
        "could not be converted into 'number'",
    );
}

#[test]
fn narrowing_is_undone_after_the_branch() {
    type_error(
        "local function f(n: number?): number if n ~= nil then end return n end",
        "could not be converted into 'number'",
    );
}

// --------------------------------------------------------------- strict

#[test]
fn strict_infers_locals_from_initializers() {
    type_error(
        "--!strict\nlocal x = 1 x = 'two'",
        "could not be converted into 'number'",
    );
    ok("--!strict\nlocal x = 1 x = 2 return x", ExVal::Integer(2));
}

#[test]
fn strict_checks_operators() {
    type_error(
        "--!strict\nreturn 'a' + 1",
        "could not be converted into 'number' (operator +)",
    );
    type_error("--!strict\nreturn 1 < 'a'", "cannot be compared");
    type_error("--!strict\nlocal n = 5 return n.x", "Cannot index number");
    type_error(
        "--!strict\nlocal n = 5 return n()",
        "Cannot call non-function number",
    );
    type_error(
        "--!strict\nlocal function f(a: number) return a end return f(1, 2)",
        "Argument count mismatch",
    );
}

#[test]
fn strict_still_allows_dynamic_values() {
    ok(
        "--!strict\nlocal x = unknown_global return 1",
        ExVal::Integer(1),
    );
    ok(
        "--!strict\nlocal t = {} t.x = 1 return t.x",
        ExVal::Integer(1),
    );
}

#[test]
fn typed_code_runs_with_identical_semantics() {
    ok(
        "type V = {x: number, y: number}
         local function add(a: V, b: V): V return {x = a.x + b.x, y = a.y + b.y} end
         local v: V = add({x = 1, y = 2}, {x = 3, y = 4})
         return v.x * 10 + v.y",
        ExVal::Integer(46),
    );
}
