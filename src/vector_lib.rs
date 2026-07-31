//! Native functions for the `vector` feature: the `vec2`/`vec3`/`vec4` global
//! constructors and the `vec` method library (`:length()`, `:dot()`, …). Methods
//! receive the vector as `args[0]` (the implicit `self`), dispatched through the global
//! `vec` table exactly like the `string` library.

use gc_arena::Mutation;

use crate::error::SiltError;
use crate::lua::VM;
use crate::userdata::InnerResult;
use crate::value::Value;
use crate::vec::{Vec2, Vec3, Vec4};

/// Coerce the i-th argument to an f32 (numbers/integers; anything else → 0.0).
fn f32_arg(args: &[Value], i: usize) -> f32 {
    match args.get(i) {
        Some(Value::Number(n)) => *n as f32,
        Some(Value::Integer(n)) => *n as f32,
        _ => 0.0,
    }
}

fn bad_self(name: &str) -> SiltError {
    SiltError::Custom(format!("bad argument #1 to '{name}' (vector expected)"))
}

fn bad_other(name: &str) -> SiltError {
    SiltError::Custom(format!("bad argument #2 to '{name}' (same-dimension vector expected)"))
}

// ---- constructors (globals) ------------------------------------------------

pub fn vec2<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    Ok(Value::Vec2(Vec2::new(f32_arg(&args, 0), f32_arg(&args, 1))))
}
pub fn vec3<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    Ok(Value::Vec3(Vec3::new(
        f32_arg(&args, 0),
        f32_arg(&args, 1),
        f32_arg(&args, 2),
    )))
}
pub fn vec4<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    Ok(Value::Vec4(Vec4::new(
        f32_arg(&args, 0),
        f32_arg(&args, 1),
        f32_arg(&args, 2),
        f32_arg(&args, 3),
    )))
}

// ---- methods (`vec` library; self = args[0]) -------------------------------

/// `v:length()` — Euclidean length, any dimension.
pub fn length<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    let n = match args.first() {
        Some(Value::Vec2(v)) => v.length(),
        Some(Value::Vec3(v)) => v.length(),
        Some(Value::Vec4(v)) => v.length(),
        _ => return Err(bad_self("length")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:length_squared()` — length², cheaper (no sqrt).
pub fn length_squared<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    let n = match args.first() {
        Some(Value::Vec2(v)) => v.length_squared(),
        Some(Value::Vec3(v)) => v.length_squared(),
        Some(Value::Vec4(v)) => v.length_squared(),
        _ => return Err(bad_self("length_squared")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:normalize()` — unit vector in the same direction; returns a new vector.
pub fn normalize<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    match args.first() {
        Some(Value::Vec2(v)) => Ok(Value::Vec2(Vec2(v.normalize()))),
        Some(Value::Vec3(v)) => Ok(Value::Vec3(Vec3(v.normalize()))),
        Some(Value::Vec4(v)) => Ok(Value::Vec4(Vec4(v.normalize()))),
        _ => Err(bad_self("normalize")),
    }
}

/// `v:dot(w)` — dot product; both operands must be the same dimension.
pub fn dot<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    let n = match (args.first(), args.get(1)) {
        (Some(Value::Vec2(a)), Some(Value::Vec2(b))) => a.dot(b.0),
        (Some(Value::Vec3(a)), Some(Value::Vec3(b))) => a.dot(b.0),
        (Some(Value::Vec4(a)), Some(Value::Vec4(b))) => a.dot(b.0),
        (Some(Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_)), _) => return Err(bad_other("dot")),
        _ => return Err(bad_self("dot")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:distance(w)` — Euclidean distance; same dimension.
pub fn distance<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    let n = match (args.first(), args.get(1)) {
        (Some(Value::Vec2(a)), Some(Value::Vec2(b))) => a.distance(b.0),
        (Some(Value::Vec3(a)), Some(Value::Vec3(b))) => a.distance(b.0),
        (Some(Value::Vec4(a)), Some(Value::Vec4(b))) => a.distance(b.0),
        (Some(Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_)), _) => {
            return Err(bad_other("distance"))
        }
        _ => return Err(bad_self("distance")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:cross(w)` — cross product; 3D only.
pub fn cross<'l>(_: &mut VM<'l>, _: &Mutation<'l>, args: Vec<Value<'l>>) -> InnerResult<'l> {
    match (args.first(), args.get(1)) {
        (Some(Value::Vec3(a)), Some(Value::Vec3(b))) => Ok(Value::Vec3(Vec3(a.cross(b.0)))),
        (Some(Value::Vec3(_)), _) => Err(bad_other("cross")),
        _ => Err(SiltError::Custom(
            "bad argument #1 to 'cross' (vec3 expected)".into(),
        )),
    }
}
