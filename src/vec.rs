//! glam-backed 2D/3D/4D vector value types for the `vector` feature.
//!
//! These are newtype wrappers around `glam`'s f32 vectors. The wrapper is required for
//! two reasons: (1) the orphan rule forbids implementing `gc_arena::Collect` directly on
//! the foreign glam types, and (2) we want a Lua-flavored `Display` (`vec3(x, y, z)`).
//!
//! All are `Copy`, immutable value types (a `Value` variant, like a number). Arithmetic
//! returns NEW vectors; there is no in-place mutation.

use std::fmt::{Display, Formatter};
use std::ops::{Add, Deref, Div, Mul, Neg, Sub};

use gc_arena::Collect;

macro_rules! vec_wrapper {
    ($name:ident, $glam:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $name(pub $glam);

        // POD — no `Gc` pointers inside, so there is nothing to trace. Mirrors the
        // `UDVec` impl in `src/lua.rs`.
        unsafe impl Collect for $name {
            fn needs_trace() -> bool {
                false
            }
            fn trace(&self, _cc: &gc_arena::Collection) {}
        }

        impl Deref for $name {
            type Target = $glam;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl From<$glam> for $name {
            fn from(v: $glam) -> Self {
                Self(v)
            }
        }
        impl From<$name> for $glam {
            fn from(v: $name) -> Self {
                v.0
            }
        }

        // Component-wise arithmetic (delegates to glam).
        impl Add for $name {
            type Output = Self;
            fn add(self, r: Self) -> Self {
                Self(self.0 + r.0)
            }
        }
        impl Sub for $name {
            type Output = Self;
            fn sub(self, r: Self) -> Self {
                Self(self.0 - r.0)
            }
        }
        impl Mul for $name {
            type Output = Self;
            fn mul(self, r: Self) -> Self {
                Self(self.0 * r.0)
            }
        }
        impl Div for $name {
            type Output = Self;
            fn div(self, r: Self) -> Self {
                Self(self.0 / r.0)
            }
        }
        impl Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }
        // Scalar multiply/divide, both operand orders.
        impl Mul<f32> for $name {
            type Output = Self;
            fn mul(self, s: f32) -> Self {
                Self(self.0 * s)
            }
        }
        impl Div<f32> for $name {
            type Output = Self;
            fn div(self, s: f32) -> Self {
                Self(self.0 / s)
            }
        }
        impl Mul<$name> for f32 {
            type Output = $name;
            fn mul(self, v: $name) -> $name {
                $name(self * v.0)
            }
        }
    };
}

vec_wrapper!(Vec2, glam::Vec2);
vec_wrapper!(Vec3, glam::Vec3);
vec_wrapper!(Vec4, glam::Vec4);

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self(glam::Vec2::new(x, y))
    }
}
impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(glam::Vec3::new(x, y, z))
    }
}
impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self(glam::Vec4::new(x, y, z, w))
    }
}

impl Display for Vec2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vec2({}, {})", self.0.x, self.0.y)
    }
}
impl Display for Vec3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vec3({}, {}, {})", self.0.x, self.0.y, self.0.z)
    }
}
impl Display for Vec4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "vec4({}, {}, {}, {})", self.0.x, self.0.y, self.0.z, self.0.w)
    }
}
