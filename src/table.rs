use std::collections::HashMap;

use gc_arena::{Collect, Mutation};

use crate::{
    error::SiltError,
    hash::FxMap,
    userdata::MetaMethod,
    value::{ExVal, FromLua, ToLua, Value},
    VM,
};

/// A Lua table with PUC-Lua's layout: a dense **array part** for the sequence
/// `1..=array.len()` and a **hash part** for everything else.
///
/// Invariants:
/// * An integer key `k` with `1 <= k <= array.len()` lives in `array[k-1]` and is
///   never in `hash`. `array` may hold `Nil` *holes* (a mid-sequence `t[k] = nil`,
///   or a `nil` in a constructor) — reads of a hole return `None` like an absent
///   key — but it is trimmed whenever its last slot becomes `Nil`.
/// * `hash` never holds the key `array.len() + 1`: appending runs `absorb`, which
///   pulls any such run of keys over, so a sequence built out of order
///   (`t[3]=c; t[1]=a; t[2]=b`) still collapses into the array.
///
/// `#t` is the border `array.len()` (after trimming), O(1) for hole-free tables.
/// Sequence reads/writes are a bounds-checked index instead of a hash and
/// probe, and a 32-byte key is no longer stored per element.
#[derive(Collect)]
#[collect(no_drop)]
pub struct Table<'v> {
    array: Vec<Value<'v>>,
    hash: FxMap<Value<'v>, Value<'v>>,
    meta: Option<Value<'v>>,
    id: usize,
}

impl<'v> Table<'v> {
    pub fn new(id: usize) -> Self {
        Table {
            array: Vec::new(),
            hash: FxMap::default(),
            meta: None,
            id,
        }
    }

    pub fn wrap_map(
        vm: &mut VM<'v>,
        mc: &Mutation<'v>,
        id: usize,
        input: &ExTable,
    ) -> Result<Table<'v>, SiltError> {
        let mut t = Table::new(id);
        for (k, v) in input.into_iter() {
            let kk: Value = k.into_value(vm, mc)?;
            let vv = v.into_value(vm, mc)?;
            t.set(kk, vv);
        }
        Ok(t)
    }

    /// Array slot for integer key `i`, if it is in the array range.
    #[inline]
    fn slot(&self, i: i64) -> Option<usize> {
        if i >= 1 && (i as usize) <= self.array.len() {
            Some(i as usize - 1)
        } else {
            None
        }
    }

    /// The value at `key`, or `None` when absent (a `Nil` hole counts as absent).
    #[inline]
    fn get_ref(&self, key: &Value<'v>) -> Option<&Value<'v>> {
        if let Value::Integer(i) = key {
            if let Some(idx) = self.slot(*i) {
                let v = &self.array[idx];
                return if matches!(v, Value::Nil) {
                    None
                } else {
                    Some(v)
                };
            }
        }
        self.hash.get(key)
    }

    /// Drop trailing `Nil`s so `array.last()` is a real value (or the array is empty).
    #[inline]
    fn trim(&mut self) {
        while matches!(self.array.last(), Some(Value::Nil)) {
            self.array.pop();
        }
    }

    /// After the array grew, pull over any `array.len() + 1, +2, …` keys that had
    /// landed in the hash part because they arrived out of order.
    #[inline]
    fn absorb(&mut self) {
        if self.hash.is_empty() {
            return;
        }
        while let Some(v) = self
            .hash
            .remove(&Value::Integer(self.array.len() as i64 + 1))
        {
            self.array.push(v);
        }
    }

    /// Store `v` at integer key `i`, routing between the two parts.
    fn set_int(&mut self, i: i64, v: Value<'v>) -> Option<Value<'v>> {
        let is_nil = matches!(v, Value::Nil);
        if let Some(idx) = self.slot(i) {
            let old = std::mem::replace(&mut self.array[idx], v);
            if is_nil && idx + 1 == self.array.len() {
                self.trim();
            }
            return if matches!(old, Value::Nil) {
                None
            } else {
                Some(old)
            };
        }
        if i as usize == self.array.len() + 1 && i >= 1 {
            if is_nil {
                return self.hash.remove(&Value::Integer(i));
            }
            self.array.push(v);
            self.absorb();
            return None;
        }
        if is_nil {
            self.hash.remove(&Value::Integer(i))
        } else {
            self.hash.insert(Value::Integer(i), v)
        }
    }

    // same as get but accepts reference Into<&Value> which is better
    pub fn getr<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<&'f Value<'v>>,
    {
        self.get_ref(key.into())
    }

    pub fn get<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        self.get_ref(&key.into())
    }

    pub fn getn(&self, i: usize) -> Option<&Value<'v>> {
        self.get_ref(&Value::Integer(i as i64))
    }

    pub fn get_value(&self, key: &Value<'v>) -> Value<'v> {
        match self.get_ref(key) {
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }

    /// Stateless iteration step for `next`/`pairs`. Given the previous key
    /// (`Nil` to start), return the next `(key, value)`: the array part in
    /// index order (skipping holes), then the hash part in its (unspecified but
    /// stable) order. `None` when exhausted.
    pub fn next_entry(&self, key: &Value<'v>) -> Option<(Value<'v>, Value<'v>)> {
        // Where to resume in the array part, if the key points into it.
        let array_from = match key {
            Value::Nil => Some(0),
            Value::Integer(i) => self.slot(*i).map(|idx| idx + 1),
            _ => None,
        };
        if let Some(from) = array_from {
            for (idx, v) in self.array.iter().enumerate().skip(from) {
                if !matches!(v, Value::Nil) {
                    return Some((Value::Integer(idx as i64 + 1), v.clone()));
                }
            }
            // array exhausted: fall through to the start of the hash part
            return self.hash.iter().next().map(|(k, v)| (k.clone(), v.clone()));
        }
        // advance past `key` in the hash part, then yield the following entry
        let mut iter = self.hash.iter();
        for (k, _) in iter.by_ref() {
            if k == key {
                break;
            }
        }
        iter.next().map(|(k, v)| (k.clone(), v.clone()))
    }

    pub fn try_get_type<'f, T, R>(&self, key: T, vm: &VM<'v>, mc: &Mutation<'v>) -> Option<R>
    where
        'v: 'f,
        T: Into<Value<'v>>,
        R: FromLua<'v>,
        R: Default,
    {
        self.get_ref(&key.into())
            .map(|v| R::from_lua(v, vm, mc).unwrap_or_default())
    }
    pub fn get_type<'f, T, R>(&self, key: T, vm: &VM<'v>, mc: &Mutation<'v>) -> R
    where
        'v: 'f,
        T: Into<Value<'v>>,
        R: FromLua<'v>,
        R: Default,
    {
        match self.get_ref(&key.into()) {
            Some(v) => R::from_lua(v, vm, mc).unwrap_or_default(),
            None => R::default(),
        }
    }

    pub fn get_number<'f, T>(&self, key: T) -> f64
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        match self.get_ref(&key.into()) {
            Some(v) => v.into(),
            _ => 0.,
        }
    }

    /// `t[key] = val`. A `nil` value removes the key. Returns the previous value.
    pub fn set<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        let k = key.into();
        let v = val.into();
        if let Value::Integer(i) = k {
            return self.set_int(i, v);
        }
        if matches!(v, Value::Nil) {
            self.hash.remove(&k)
        } else {
            self.hash.insert(k, v)
        }
    }

    /// Constructor positional element: `{a, b, c}` appends in order. A `nil`
    /// here keeps its position (`{1, nil, 3}` puts 3 at index 3).
    pub fn raw_push<'f, V>(&mut self, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        V: Into<Value<'v>>,
    {
        self.array.push(val.into());
        self.absorb();
        None
    }

    pub fn set_and_check<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        // `absorb` keeps the border right for out-of-order builds; nothing
        // extra to check any more.
        self.set(key, val)
    }

    pub fn to_array<'f, T, const N: usize>(&self) -> [T; N]
    where
        'v: 'f,
        T: Default,
        T: Copy,
        T: From<Value<'v>>,
    {
        let mut out: [T; N] = [T::default(); N];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = match self.getn(i + 1) {
                Some(v) => T::from(v.clone()),
                None => T::default(),
            };
        }
        out
    }

    /// The sequence `1..n` up to the first missing index, like `ipairs`.
    pub fn to_vec<'f, T>(&self, vm: &VM<'v>, mc: &Mutation<'v>) -> Vec<T>
    where
        'v: 'f,
        T: Default,
        T: FromLua<'v>,
    {
        self.array
            .iter()
            .take_while(|v| !matches!(v, Value::Nil))
            .map(|v| T::from_lua(v, vm, mc).unwrap_or_default())
            .collect()
    }

    pub fn to_exval(&self) -> ExTable {
        let mut map = HashMap::new();
        for (k, v) in self.iter() {
            map.insert(k.into(), v.clone().into());
        }
        ExTable {
            id: self.id,
            data: map,
        }
    }

    /// Lua's `#t`: the array border.
    pub fn len(&self) -> usize {
        let mut n = self.array.len();
        while n > 0 && matches!(self.array[n - 1], Value::Nil) {
            n -= 1;
        }
        n
    }

    pub fn is_empty(&self) -> bool {
        self.array.iter().all(|v| matches!(v, Value::Nil)) && self.hash.is_empty()
    }

    /// Positional insert — Lua `table.insert(t, pos, v)`: open a slot at `pos` by
    /// shifting every element from `pos` up one index, then store `v` there.
    pub fn insert(&mut self, key: Value<'v>, value: Value<'v>) -> Result<Value<'v>, SiltError> {
        let i = key.strict_int()?;
        if i >= 1 && (i as usize) <= self.array.len() + 1 {
            self.array.insert(i as usize - 1, value);
            self.absorb();
        } else {
            self.set_int(i, value);
        }
        Ok(Value::Nil)
    }

    /// Append at the border (`t[#t + 1] = v`).
    pub fn push(&mut self, value: Value<'v>) {
        let b = self.len() as i64;
        self.set_int(b + 1, value);
    }

    /// The current array border (last integer index in use, 0 when empty) — Lua's `#t`.
    pub fn border(&self) -> i64 {
        self.len() as i64
    }

    /// Remove the element at `pos` (Lua `table.remove`): return it and shift every
    /// element above it down one index to close the gap.
    pub fn remove_at(&mut self, pos: i64) -> Value<'v> {
        if let Some(idx) = self.slot(pos) {
            let removed = self.array.remove(idx);
            self.trim();
            removed
        } else {
            self.hash.remove(&Value::Integer(pos)).unwrap_or_default()
        }
    }

    pub fn pop(&mut self) -> Value<'v> {
        let b = self.len();
        if b == 0 {
            return Value::Nil;
        }
        self.array.truncate(b);
        let v = self.array.pop().unwrap_or_default();
        self.trim();
        v
    }

    pub fn concat_array<A, I>(&mut self, array: I)
    where
        I: IntoIterator<Item = A>,
        A: Into<Value<'v>>,
    {
        for v in array.into_iter() {
            self.push(v.into());
        }
    }

    pub fn concat_complex_array<A, I>(&mut self, lua: &VM<'v>, mc: &Mutation<'v>, array: I)
    where
        I: IntoIterator<Item = A>,
        A: ToLua<'v>,
    {
        for v in array.into_iter() {
            let res = v.to_lua(lua, mc).unwrap_or_default();
            self.push(res);
        }
    }

    pub fn set_metatable(&mut self, metatable: Value<'v>) {
        self.meta = Some(metatable);
    }

    pub fn get_metatable(&self) -> Value<'v> {
        self.meta.clone().unwrap_or(Value::Nil)
    }

    /// The raw `__index` metafield (a table or a function), if this table has a
    /// metatable that defines one. Unlike [`by_meta_method`], this does not require
    /// the value to be callable — `__index` is most often a table (the OOP class
    /// pattern). Returns `None` when there is no metatable or no `__index`.
    pub fn meta_index(&self) -> Option<Value<'v>> {
        if let Some(Value::Table(mt)) = &self.meta {
            let v = mt.borrow().get_value(&Value::String("__index".to_string()));
            if !matches!(v, Value::Nil) {
                return Some(v);
            }
        }
        None
    }

    pub fn by_meta_method(&self, method: MetaMethod) -> Result<Value<'v>, SiltError> {
        if let Some(Value::Table(t)) = &self.meta {
            if let Some(func) = t
                .borrow()
                .get(Value::String(method.as_table_key().to_string()))
            {
                return if let Value::Closure(_) = func {
                    Ok(func.clone())
                } else {
                    Err(SiltError::MetaMethodNotCallable(method))
                };
            }
        }
        Err(SiltError::MetaMethodMissing(method))
    }

    /// Every `(key, value)` pair: the array part (holes skipped) in index order,
    /// then the hash part.
    pub fn iter(&self) -> impl Iterator<Item = (Value<'v>, &Value<'v>)> + '_ {
        self.array
            .iter()
            .enumerate()
            .filter(|(_, v)| !matches!(v, Value::Nil))
            .map(|(i, v)| (Value::Integer(i as i64 + 1), v))
            .chain(self.hash.iter().map(|(k, v)| (k.clone(), v)))
    }

    pub fn list_keys(&self) -> String {
        self.iter()
            .map(|(k, _)| k.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}

impl ToString for Table<'_> {
    fn to_string(&self) -> String {
        format!(
            "table{}[{}]{{{}}}",
            self.id,
            self.iter().count(),
            self.iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

#[derive(Debug, Clone)]
pub struct ExTable {
    id: usize,
    data: HashMap<ExVal, ExVal>,
}

impl ExTable {
    pub fn getn(&self, i: usize) -> Option<&ExVal> {
        self.data.get(&ExVal::Integer(i as i64))
    }

    pub fn pop_value(&mut self, i: usize) -> ExVal {
        self.data
            .remove(&ExVal::Integer(i as i64))
            .unwrap_or(ExVal::Nil)
    }

    pub fn get(&self, field: &str) -> Option<&ExVal> {
        self.data.get(&ExVal::String(field.to_owned()))
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
    // pub fn iter(&self) -> Iter<'_, ExVal, ExVal> {
    //     self.data.iter()
    // }
}

impl PartialEq for ExTable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ToString for ExTable {
    fn to_string(&self) -> String {
        format!(
            "table{}[{}]{{{}}}",
            self.id,
            self.data.len(),
            self.data
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}
// TODO proper immutable iterator
// impl Iterator for ExTable {
//     type Item = (ExVal, ExVal);
//     type IntoIter = std::collections::hash_map::IntoIter<ExVal, ExVal>;
//
//     fn into_iter(self) -> Self::IntoIter {
//         self.data.into_iter()
//     }
// }
impl IntoIterator for ExTable {
    type Item = (ExVal, ExVal);
    type IntoIter = std::collections::hash_map::IntoIter<ExVal, ExVal>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<'a> IntoIterator for &'a ExTable {
    type Item = (&'a ExVal, &'a ExVal);
    type IntoIter = std::collections::hash_map::Iter<'a, ExVal, ExVal>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<'a> IntoIterator for &'a mut ExTable {
    type Item = (&'a ExVal, &'a mut ExVal);
    type IntoIter = std::collections::hash_map::IterMut<'a, ExVal, ExVal>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

impl<A, B> From<ExVal> for (A, B)
where
    A: From<ExVal>,
    B: From<ExVal>,
{
    fn from(value: ExVal) -> Self {
        match value {
            ExVal::Table(mut t) => (&mut t).into(),
            _ => (ExVal::Nil.into(), ExVal::Nil.into()),
        }
    }
}

impl<A, B> From<&mut ExVal> for (A, B)
where
    A: From<ExVal>,
    B: From<ExVal>,
{
    fn from(value: &mut ExVal) -> Self {
        match value {
            ExVal::Table(t) => t.into(),
            _ => (ExVal::Nil.into(), ExVal::Nil.into()),
        }
    }
}
impl<A, B> From<&mut ExTable> for (A, B)
where
    A: From<ExVal>,
    B: From<ExVal>,
{
    fn from(value: &mut ExTable) -> Self {
        (value.pop_value(0).into(), value.pop_value(1).into())
    }
}
