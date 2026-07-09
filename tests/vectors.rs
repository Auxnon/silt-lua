//! glam-backed vector value type (`vector` feature). Run with `--features vector`.

use silt_lua::gc_arena::Mutation;
use silt_lua::glam;
use silt_lua::userdata::{InnerResult, UserData, UserDataFields};
use silt_lua::{simple, Compiler, ExVal, Lua, Value, VM};

/// Extract a numeric result as f64 (vectors return components/lengths as numbers).
fn num(src: &str) -> f64 {
    match simple(src) {
        ExVal::Number(n) => n,
        ExVal::Integer(i) => i as f64,
        other => panic!("expected a number from {src:?}, got {other:?}"),
    }
}

fn str_of(src: &str) -> String {
    match simple(src) {
        ExVal::String(s) => s,
        other => panic!("expected a string from {src:?}, got {other:?}"),
    }
}

const EPS: f64 = 1e-5;

// ---- construction + field access -------------------------------------------

#[test]
fn construct_and_read_fields() {
    assert_eq!(num("local v = vec2(1, 2) return v.x"), 1.0);
    assert_eq!(num("local v = vec2(1, 2) return v.y"), 2.0);
    assert_eq!(num("local v = vec3(1, 2, 3) return v.z"), 3.0);
    assert_eq!(num("local v = vec4(1, 2, 3, 4) return v.w"), 4.0);
    assert_eq!(num("local v = vec3(1, 2, 3) return v.x + v.y + v.z"), 6.0);
}

#[test]
fn type_name_is_dimension_specific() {
    assert_eq!(str_of("return type(vec2(1, 2))"), "vec2");
    assert_eq!(str_of("return type(vec3(1, 2, 3))"), "vec3");
    assert_eq!(str_of("return type(vec4(1, 2, 3, 4))"), "vec4");
}

// ---- operators -------------------------------------------------------------

#[test]
fn component_wise_arithmetic() {
    assert_eq!(num("local c = vec3(1,2,3) + vec3(4,5,6) return c.x"), 5.0);
    assert_eq!(num("local c = vec3(4,5,6) - vec3(1,2,3) return c.z"), 3.0);
    assert_eq!(num("local c = vec2(2,3) * vec2(4,5) return c.y"), 15.0);
    assert_eq!(num("local c = vec2(8,6) / vec2(2,3) return c.x"), 4.0);
}

#[test]
fn scalar_scaling() {
    assert_eq!(num("local c = vec3(1,2,3) * 2 return c.y"), 4.0);
    assert_eq!(num("local c = 2 * vec3(1,2,3) return c.z"), 6.0);
    assert_eq!(num("local c = vec3(2,4,6) / 2 return c.x"), 1.0);
}

#[test]
fn scalar_add_is_rejected() {
    // Only `*`/`/` mix scalars with vectors (per the feature's design).
    match simple("return 1 + vec3(1,1,1)") {
        ExVal::String(s) => assert!(s.contains("failed with:"), "expected error, got {s}"),
        other => panic!("expected an error for scalar + vector, got {other:?}"),
    }
}

#[test]
fn unary_negate() {
    assert_eq!(num("local v = -vec3(1,2,3) return v.x"), -1.0);
    assert_eq!(num("local v = -vec3(1,2,3) return v.z"), -3.0);
}

#[test]
fn equality_by_value() {
    assert_eq!(simple("return vec2(1,2) == vec2(1,2)"), ExVal::Bool(true));
    assert_eq!(simple("return vec3(1,2,3) == vec3(1,2,9)"), ExVal::Bool(false));
}

// ---- methods (`vec` library) -----------------------------------------------

#[test]
fn methods_length_and_dot() {
    assert_eq!(num("return vec3(3,4,0):length()"), 5.0);
    assert_eq!(num("return vec3(3,4,0):length_squared()"), 25.0);
    assert_eq!(num("return vec2(1,0):dot(vec2(0,1))"), 0.0);
    assert_eq!(num("return vec2(2,3):dot(vec2(4,5))"), 23.0);
    assert_eq!(num("return vec3(0,3,4):distance(vec3(0,0,0))"), 5.0);
}

#[test]
fn methods_normalize_and_cross() {
    // normalize → unit length (f32 precision, so approximate).
    assert!((num("local u = vec2(3,4):normalize() return u:length()") - 1.0).abs() < EPS);
    // x cross y = z (right-handed).
    assert_eq!(num("local c = vec3(1,0,0):cross(vec3(0,1,0)) return c.z"), 1.0);
}

#[test]
fn cross_on_non_vec3_errors() {
    match simple("return vec2(1,2):cross(vec2(3,4))") {
        ExVal::String(s) => assert!(s.contains("failed with:"), "expected error, got {s}"),
        other => panic!("expected an error for vec2:cross, got {other:?}"),
    }
}

// ---- entity interop: userdata field typed as a glam::Vec3 ------------------

struct Ent {
    pos: glam::Vec3,
}

impl UserData for Ent {
    fn type_name() -> &'static str {
        "Ent"
    }
    fn get_id(&self) -> usize {
        1
    }
    fn add_fields<'v, F: UserDataFields<'v, Self>>(f: &mut F) {
        // Getter returns the entity's position AS a vector (glam::Vec3 -> Value::Vec3).
        f.add_field_method_get("pos", |_vm, _mc, e| Ok(e.pos));
        // Setter accepts a vector and stores it (Value::Vec3 -> glam::Vec3).
        f.add_field_method_set("pos", |_vm, _mc, e, v: glam::Vec3| {
            e.pos = v;
            Ok(())
        });
    }
}

fn make_ent<'l>(vm: &mut VM<'l>, mc: &Mutation<'l>, _: Vec<Value<'l>>) -> InnerResult<'l> {
    Ok(vm.create_userdata(mc, Ent { pos: glam::Vec3::new(1.0, 2.0, 3.0) }))
}

fn run_ent(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    let mut comp = Compiler::new();
    lua.enter(|vm, mc| {
        vm.register_native_function(mc, "make_ent", make_ent);
    });
    lua.run(None, src, &mut comp).map_err(|e| e.to_string()).unwrap()
}

#[test]
fn entity_vector_field_roundtrip() {
    // Read the entity's pos as a vector, operate immutably, write it back.
    let out = run_ent(
        r#"
        local e = make_ent()
        e.pos = e.pos + vec3(0, 10, 0)
        local p = e.pos
        return p.y
    "#,
    );
    assert_eq!(out, ExVal::Number(12.0)); // started at y=2, +10
}

#[test]
fn entity_reads_initial_vector() {
    let out = run_ent("local e = make_ent() local p = e.pos return p.x + p.z");
    assert_eq!(out, ExVal::Number(4.0)); // 1 + 3
}
