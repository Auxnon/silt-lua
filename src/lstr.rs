//! Lua strings: immutable, garbage-collected, hashed once on first use as a key.
//!
//! `Value::String` holds a `Gc<LuaStr>` rather than an owned `String`, so a
//! `Value` is a plain `Copy` word pair: pushing, popping and overwriting stack
//! slots never runs drop glue or allocates, and table lookups reuse the cached
//! hash. A string is built once (literal, concat, native result) and then only
//! its pointer moves.

use std::{
    cell::Cell,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
};

use gc_arena::{metrics::Metrics, Collect, Gc, Mutation};

use crate::hash::FxHasher;

#[derive(Collect)]
#[collect(require_static)]
pub struct LuaStr {
    /// `hash_str` of the text, filled in by the first `key_hash` (0 = not yet).
    /// Most strings (concat results, messages) are never keys, so building one
    /// doesn't pay a pass over its bytes.
    hash: Cell<u64>,
    s: Box<str>,
    /// The arena's metrics: the text lives outside the `Gc` allocation, so it is
    /// reported as external memory (and returned on drop) to pace collection.
    metrics: Metrics,
}

pub type GcStr<'gc> = Gc<'gc, LuaStr>;

/// The hash a string key carries in tables, for `LuaStr` and `&str` alike.
/// Never 0, which `LuaStr` reserves for "not computed".
#[inline]
pub fn hash_str(s: &str) -> u64 {
    let mut h = FxHasher::default();
    h.write(s.as_bytes());
    // fold the length in so "" and a run of zero bytes differ
    h.write_usize(s.len());
    h.finish().max(1)
}

impl LuaStr {
    /// Allocate `s` as a collected string.
    #[inline]
    pub fn gc<'gc>(mc: &Mutation<'gc>, s: impl Into<Box<str>>) -> GcStr<'gc> {
        let s = s.into();
        let metrics = mc.metrics().clone();
        metrics.mark_external_allocation(s.len());
        Gc::new(mc, LuaStr { hash: Cell::new(0), s, metrics })
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.s
    }

    #[inline]
    pub fn key_hash(&self) -> u64 {
        match self.hash.get() {
            0 => {
                let h = hash_str(&self.s);
                self.hash.set(h);
                h
            }
            h => h,
        }
    }
}

impl Drop for LuaStr {
    fn drop(&mut self) {
        self.metrics.mark_external_deallocation(self.s.len());
    }
}

impl Deref for LuaStr {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.s
    }
}

impl PartialEq for LuaStr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.s == other.s
    }
}

impl PartialEq<str> for LuaStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        &*self.s == other
    }
}

impl PartialOrd for LuaStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.s.partial_cmp(&other.s)
    }
}

impl Hash for LuaStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.key_hash());
    }
}

impl fmt::Display for LuaStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.s)
    }
}

impl fmt::Debug for LuaStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.s, f)
    }
}

/// Look a table up by `&str` without allocating a `LuaStr`: hashes and compares
/// like a `Value::String` key (see `Value`'s `Hash`).
pub struct StrKey<'a> {
    pub s: &'a str,
    pub hash: u64,
}

impl<'a> StrKey<'a> {
    #[inline]
    pub fn new(s: &'a str) -> Self {
        StrKey { s, hash: hash_str(s) }
    }
}

impl Hash for StrKey<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}
