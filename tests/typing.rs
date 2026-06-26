// Phase 1 of static typing (cargo feature `typing`, off by default). At this
// phase annotations are PARSED and TRACKED but not yet checked — so typed code
// must still compile and run with identical dynamic semantics. Run with:
//   cargo test --features typing --test typing
#![cfg(feature = "typing")]
use silt_lua::types::{assignable, Type};
use silt_lua::{simple, valeq, ExVal};

#[test]
fn typed_local_runs_dynamically() {
    valeq!("local x: number = 5 return x + 1", ExVal::Integer(6));
    valeq!("local s: string = 'hi' return s:upper()", ExVal::String("HI".to_string()));
    valeq!("local t: table = {} t.x = 9 return t.x", ExVal::Integer(9));
}

#[test]
fn typed_multivar_local() {
    valeq!("local a: number, b: number = 1, 2 return a + b", ExVal::Integer(3));
}

#[test]
fn typed_function_params() {
    valeq!(
        "local function add(a: number, b: number) return a + b end return add(3, 4)",
        ExVal::Integer(7)
    );
}

#[test]
fn unknown_named_type_is_accepted() {
    // an unresolved named type is tracked as `Named` and treated as `any`
    valeq!("local x: Foo = 5 return x", ExVal::Integer(5));
}

#[test]
fn annotation_not_confused_with_method_call() {
    // `:` in expression position is still a method call, not an annotation
    valeq!(
        "local t = {n = 3} function t:get() return self.n end return t:get()",
        ExVal::Integer(3)
    );
}

#[test]
fn assignable_lattice() {
    // exact matches
    assert!(assignable(&Type::Number, &Type::Number));
    assert!(!assignable(&Type::Number, &Type::String));
    // gradual: Any is compatible both ways
    assert!(assignable(&Type::Number, &Type::Any));
    assert!(assignable(&Type::Any, &Type::String));
    // unresolved named types are lenient for now
    assert!(assignable(&Type::Number, &Type::Named("Foo".into())));
    // optional accepts nil and its base
    let opt = Type::Optional(Box::new(Type::Number));
    assert!(assignable(&opt, &Type::Nil));
    assert!(assignable(&opt, &Type::Number));
}

#[test]
fn type_from_name() {
    assert_eq!(Type::from_name("number"), Type::Number);
    assert_eq!(Type::from_name("string"), Type::String);
    assert_eq!(Type::from_name("boolean"), Type::Boolean);
    assert_eq!(Type::from_name("any"), Type::Any);
    assert_eq!(Type::from_name("Widget"), Type::Named("Widget".to_string()));
}
