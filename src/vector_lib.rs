//! Native functions for the `vector` feature: the `vec2`/`vec3`/`vec4` global
//! constructors and the `vec` method library (`:length()`, `:dot()`, …). Methods
//! receive the vector as their first argument (the implicit `self`), dispatched
//! through the `vec` table exactly like the `string` library.
//!
//! Arguments arrive through the typed `FromLuaMulti` conversions: a tuple reads the
//! call's argument slice in place, where `Vec<Value>` would copy it into a fresh
//! allocation on every call.

use gc_arena::Mutation;

use crate::error::SiltError;
use crate::lua::VM;
use crate::userdata::InnerResult;
use crate::value::Value;
use crate::vec::{Vec2, Vec3, Vec4};

fn bad_self(name: &str) -> SiltError {
    SiltError::Custom(format!("bad argument #1 to '{name}' (vector expected)"))
}

fn bad_other(name: &str) -> SiltError {
    SiltError::Custom(format!("bad argument #2 to '{name}' (same-dimension vector expected)"))
}

// ---- constructors (globals) ------------------------------------------------
// Components convert like any `f32` argument: numbers and integers, anything else 0.

pub fn vec2<'l>(_: &mut VM<'l>, _: &Mutation<'l>, (x, y): (f32, f32)) -> InnerResult<'l> {
    Ok(Value::Vec2(Vec2::new(x, y)))
}
pub fn vec3<'l>(_: &mut VM<'l>, _: &Mutation<'l>, (x, y, z): (f32, f32, f32)) -> InnerResult<'l> {
    Ok(Value::Vec3(Vec3::new(x, y, z)))
}
pub fn vec4<'l>(
    _: &mut VM<'l>,
    _: &Mutation<'l>,
    (x, y, z, w): (f32, f32, f32, f32),
) -> InnerResult<'l> {
    Ok(Value::Vec4(Vec4::new(x, y, z, w)))
}

// ---- methods (`vec` library; the first argument is self) -------------------

/// `v:length()` — Euclidean length, any dimension.
pub fn length<'l>(_: &mut VM<'l>, _: &Mutation<'l>, v: Value<'l>) -> InnerResult<'l> {
    let n = match v {
        Value::Vec2(v) => v.length(),
        Value::Vec3(v) => v.length(),
        Value::Vec4(v) => v.length(),
        _ => return Err(bad_self("length")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:length_squared()` — length², cheaper (no sqrt).
pub fn length_squared<'l>(_: &mut VM<'l>, _: &Mutation<'l>, v: Value<'l>) -> InnerResult<'l> {
    let n = match v {
        Value::Vec2(v) => v.length_squared(),
        Value::Vec3(v) => v.length_squared(),
        Value::Vec4(v) => v.length_squared(),
        _ => return Err(bad_self("length_squared")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:normalize()` — unit vector in the same direction; returns a new vector.
pub fn normalize<'l>(_: &mut VM<'l>, _: &Mutation<'l>, v: Value<'l>) -> InnerResult<'l> {
    match v {
        Value::Vec2(v) => Ok(Value::Vec2(Vec2(v.normalize()))),
        Value::Vec3(v) => Ok(Value::Vec3(Vec3(v.normalize()))),
        Value::Vec4(v) => Ok(Value::Vec4(Vec4(v.normalize()))),
        _ => Err(bad_self("normalize")),
    }
}

/// `v:dot(w)` — dot product; both operands must be the same dimension.
pub fn dot<'l>(_: &mut VM<'l>, _: &Mutation<'l>, (a, b): (Value<'l>, Value<'l>)) -> InnerResult<'l> {
    let n = match (a, b) {
        (Value::Vec2(a), Value::Vec2(b)) => a.dot(b.0),
        (Value::Vec3(a), Value::Vec3(b)) => a.dot(b.0),
        (Value::Vec4(a), Value::Vec4(b)) => a.dot(b.0),
        (Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_), _) => return Err(bad_other("dot")),
        _ => return Err(bad_self("dot")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:distance(w)` — Euclidean distance; same dimension.
pub fn distance<'l>(_: &mut VM<'l>, _: &Mutation<'l>, (a, b): (Value<'l>, Value<'l>)) -> InnerResult<'l> {
    let n = match (a, b) {
        (Value::Vec2(a), Value::Vec2(b)) => a.distance(b.0),
        (Value::Vec3(a), Value::Vec3(b)) => a.distance(b.0),
        (Value::Vec4(a), Value::Vec4(b)) => a.distance(b.0),
        (Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_), _) => {
            return Err(bad_other("distance"))
        }
        _ => return Err(bad_self("distance")),
    };
    Ok(Value::Number(n as f64))
}

/// `v:cross(w)` — cross product; 3D only.
pub fn cross<'l>(_: &mut VM<'l>, _: &Mutation<'l>, (a, b): (Value<'l>, Value<'l>)) -> InnerResult<'l> {
    match (a, b) {
        (Value::Vec3(a), Value::Vec3(b)) => Ok(Value::Vec3(Vec3(a.cross(b.0)))),
        (Value::Vec3(_), _) => Err(bad_other("cross")),
        _ => Err(SiltError::Custom(
            "bad argument #1 to 'cross' (vec3 expected)".into(),
        )),
    }
}
