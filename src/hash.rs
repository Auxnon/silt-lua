//! Non-cryptographic hashers for the interpreter's internal maps.
//!
//! `std`'s default SipHash is DoS-resistant but pays a per-lookup setup cost that
//! dominates when the keys are short identifiers or small integers looked up on
//! every instruction (table keys, userdata field names). Nothing here is exposed
//! to untrusted remote input, so the trade is worth it.

use std::{
    any::TypeId,
    collections::HashMap,
    hash::{BuildHasherDefault, Hasher},
};

/// FxHash (rustc's hasher): one multiply-rotate per word.
#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl FxHasher {
    const K: u64 = 0x517c_c1b7_2722_0a95;
    #[inline]
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(Self::K);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut b = bytes;
        while b.len() >= 8 {
            self.add(u64::from_le_bytes(b[..8].try_into().unwrap()));
            b = &b[8..];
        }
        if b.len() >= 4 {
            self.add(u32::from_le_bytes(b[..4].try_into().unwrap()) as u64);
            b = &b[4..];
        }
        for &x in b {
            self.add(x as u64);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.add(i as u64);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

/// `TypeId` is already a hash; pass its bits through instead of re-hashing.
#[derive(Default, Clone, Copy)]
pub struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // TypeId hashes as one u128 (two u64 writes on older toolchains); fold
        // whatever arrives.
        for chunk in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            self.0 ^= u64::from_le_bytes(w);
        }
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 ^= i;
    }
    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.0 ^= (i as u64) ^ ((i >> 64) as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

pub type FxBuild = BuildHasherDefault<FxHasher>;
pub type FxMap<K, V> = HashMap<K, V, FxBuild>;
pub type TypeIdMap<V> = HashMap<TypeId, V, BuildHasherDefault<TypeIdHasher>>;
