use std::{
    collections::{hash_map::Iter, HashMap},
    usize,
};

use gc_arena::{Collect, Mutation};

use crate::{
    error::SiltError,
    userdata::MetaMethod,
    value::{ExVal, FromLua, ToLua, Value},
    VM,
};

#[derive(Collect)]
#[collect(no_drop)]
pub struct Table<'v> {
    data: HashMap<Value<'v>, Value<'v>>,
    meta: Option<Value<'v>>,
    // data: RefLock<HashMap<String, String>>,
    /** replicate standard lua behavior */
    counter: i64,
    id: usize,
}

impl<'v> Table<'v> {
    pub fn new(id: usize) -> Self {
        Table {
            data: HashMap::new(),
            meta: None,
            counter: 0,
            id,
        }
    }

    pub fn wrap_map(
        vm: &mut VM<'v>,
        mc: &Mutation<'v>,
        id: usize,
        input: &ExTable,
    ) -> Result<Table<'v>, SiltError>
// where
        // T: ToLua<'v>,
    {
        let mut data = HashMap::new();
        for (k, v) in input.into_iter() {
            let kk: Value = k.into_value(vm, mc)?;
            let vv = v.into_value(vm, mc)?;
            data.insert(kk, vv);
        }
        Ok(Table {
            data,
            meta: None,
            counter: 0,
            id,
        })
    }

    // same as get but accepts reference Into<&Value> which is better
    pub fn getr<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<&'f Value<'v>>,
    {
        self.data.get(key.into())
    }

    pub fn get<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        self.data.get(&key.into())
    }

    pub fn getn(&self, i: usize) -> Option<&Value<'v>> {
        self.data.get(&Value::Integer(i as i64))
    }

    pub fn get_value(&self, key: &Value<'v>) -> Value<'v> {
        let r = self.data.get(key);
        match r {
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }

    /// Stateless iteration step for `next`/`pairs`. Given the previous key
    /// (`Nil` to start), return the next `(key, value)` in the hashmap's
    /// iteration order, or `None` when exhausted. Order is unspecified (Lua
    /// makes no guarantee) but stable for an unmodified table within one pass.
    pub fn next_entry(&self, key: &Value<'v>) -> Option<(Value<'v>, Value<'v>)> {
        let mut iter = self.data.iter();
        if matches!(key, Value::Nil) {
            return iter.next().map(|(k, v)| (k.clone(), v.clone()));
        }
        // advance past `key`, then yield the following entry
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
        self.data
            .get(&key.into())
            .map(|v| R::from_lua(v, vm, mc).unwrap_or_default())
    }
    pub fn get_type<'f, T, R>(&self, key: T, vm: &VM<'v>, mc: &Mutation<'v>) -> R
    where
        'v: 'f,
        T: Into<Value<'v>>,
        R: FromLua<'v>,
        R: Default,
    {
        match self.data.get(&key.into()) {
            Some(v) => R::from_lua(v, vm, mc).unwrap_or_default(),
            None => R::default(),
        }
    }

    pub fn get_number<'f, T>(&self, key: T) -> f64
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        match self.data.get(&key.into()) {
            Some(v) => v.into(),
            _ => 0.,
        }
    }

    /// set at key without checking re-evaluating indicies
    pub fn set<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        let k=key.into();
        let v=val.into();
        // println!(" WE SET {} {}",k.clone(),v.clone());
        self.data.insert(k,v)
    }

    pub fn raw_push<'f, V>(&mut self, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        V: Into<Value<'v>>,
    {
        let key = self.border() + 1;
        self.data.insert(key.into(), val.into())
    }

    fn recursion(&mut self, i: i64) {
        let prev = i - 1;
        if self.data.contains_key(&prev.into()) {
            if self.counter == prev {
                self.counter = i;
            } else {
                self.recursion(prev);
            }
        }
    }

    pub fn set_and_check<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        let key = key.into();
        if let Ok(i) = key.strict_int() {
            if i >= self.counter {
                self.recursion(i);
            }
        }
        self.data.insert(key, val.into())
    }

    pub fn to_array<'f, T, const N: usize>(&self) -> [T; N]
    where
        'v: 'f,
        T: Default,
        T: Copy,
        T: From<Value<'v>>,
    {
        // Read the array part by index (Lua is 1-indexed). Iterating `self.data`
        // (a HashMap) yields hash order, not 1..N, so `{a, b, c}` came out as an
        // arbitrary cyclic rotation that varied per table.
        let mut out: [T; N] = [T::default(); N];
        for i in 0..N {
            out[i] = match self.data.get(&Value::Integer((i + 1) as i64)) {
                Some(v) => T::from(v.clone()),
                None => T::default(),
            };
        }
        out
    }

    pub fn to_vec<'f, T>(&self, vm: &VM<'v>, mc: &Mutation<'v>) -> Vec<T>
    where
        'v: 'f,
        T: Default,
        // T: Copy,
        T: FromLua<'v>,
    {
        self.data
            .iter()
            .map(|f| T::from_lua(f.1, vm, mc).unwrap_or_default())
            .collect()
    }

    pub fn to_exval(&self) -> ExTable {
        let mut map = HashMap::new();
        for (k, v) in self.data.iter() {
            map.insert(k.clone().into(), v.clone().into());
        }
        ExTable {
            id: self.id,
            data: map,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /** push by counter's current index, if it aready exists keep incrementing until empty position is found */
    pub fn insert(&mut self, key: Value<'v>, value: Value<'v>) -> Result<Value<'v>, SiltError> {
        // let key = key.strict_int()?;
        //
        // // DEV this just feels clunky to replicate lua's behavior
        // self.counter += 1;
        // let mut key = Value::Integer(self.counter);
        // while self.data.contains_key(&key) {
        //     self.counter += 1;
        //     key.force_to_int(self.counter);
        // }
        // self.data.insert(key, value);
        // Ok(())

        // Positional insert — Lua `table.insert(t, pos, v)`: open a slot at `pos` by
        // shifting every element in `pos..=counter` up one index, then store `v` there.
        // The old loop ran `counter..pos`, which is empty whenever `pos < counter`, so
        // nothing shifted and the element already at `pos` was silently overwritten.
        let i = key.strict_int()?;
        let mut k = self.border();
        while k >= i && k >= 1 {
            if let Some(v) = self.data.remove(&Value::Integer(k)) {
                self.data.insert(Value::Integer(k + 1), v);
            }
            k -= 1;
        }
        self.data.insert(Value::Integer(i), value);
        Ok(Value::Nil)
    }

    pub fn push(&mut self, value: Value<'v>) {
        // Append at the array border (Lua tables are 1-indexed, so the new element lands
        // at border + 1). Derived from contents so it works regardless of how the table
        // was grown — including a `t[#t+1] = v` loop, which routes through `set` and never
        // maintained the old `counter`.
        let key = self.border() + 1;
        self.data.insert(Value::Integer(key), value);
    }

    /// The array border — an `n` with `t[n] ~= nil` and `t[n+1] == nil` (Lua's `#t` for a
    /// hole-free table). Computed from actual contents via an unbounded binary search
    /// (exponential probe, O(log n) on the hashmap) rather than a hand-maintained counter,
    /// so `insert`/`remove_at`/`push` stay correct on tables built by index assignment —
    /// the old `counter` was only updated by the constructor path, never by `t[k] = v`.
    pub fn border(&self) -> i64 {
        let has = |i: i64| self.data.contains_key(&Value::Integer(i));
        if !has(1) {
            return 0;
        }
        // exponential probe: find j present but 2j absent
        let mut i = 1i64;
        let mut j = 2i64;
        while has(j) {
            i = j;
            if j > i64::MAX / 2 {
                // pathological (huge dense array) — fall back to a linear walk
                let mut n = i;
                while has(n + 1) {
                    n += 1;
                }
                return n;
            }
            j *= 2;
        }
        // binary search for the border in (i, j)
        while j - i > 1 {
            let m = i + (j - i) / 2;
            if has(m) {
                i = m;
            } else {
                j = m;
            }
        }
        i
    }

    /// Remove the element at `pos` (Lua `table.remove`): return it, then shift every
    /// element in `pos+1..=counter` DOWN one index to close the gap, and shrink the
    /// border. The old `remove` removed by *value*, shifted the wrong direction, and
    /// decremented the border unconditionally — it never actually removed anything
    /// (the standard-lib wrapper even called `insert` instead). See `tests/tables.rs`.
    pub fn remove_at(&mut self, pos: i64) -> Value<'v> {
        let n = self.border();
        let removed = self.data.remove(&Value::Integer(pos)).unwrap_or_default();
        let mut k = pos + 1;
        while k <= n {
            if let Some(v) = self.data.remove(&Value::Integer(k)) {
                self.data.insert(Value::Integer(k - 1), v);
            }
            k += 1;
        }
        removed
    }

    pub fn pop(&mut self) -> Value<'v> {
        let n = self.border();
        if n == 0 {
            return Value::Nil;
        }
        self.data.remove(&Value::Integer(n)).unwrap_or_default()
    }

    pub fn concat_array<A, I>(&mut self, array: I)
    where
        I: IntoIterator<Item = A>,
        A: Into<Value<'v>>,
    {
        self.counter += 1;
        for v in array.into_iter() {
            let key = Value::Integer(self.counter);
            self.data.insert(key, A::into(v));
            self.counter += 1;
        }
    }

    pub fn concat_complex_array<A, I>(&mut self, lua: &VM<'v>, mc: &Mutation<'v>, array: I)
    where
        I: IntoIterator<Item = A>,
        A: ToLua<'v>,
    {
        self.counter += 1;
        for v in array.into_iter() {
            let key = Value::Integer(self.counter);
            let res = v.to_lua(lua, mc).unwrap_or_default();
            self.data.insert(key, res);
            self.counter += 1;
        }
    }

    pub fn set_metatable(&mut self, metatable: Value<'v>) {
        // println!("setting metatable: {}", metatable);
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
            let v = mt
                .borrow()
                .get_value(&Value::String("__index".to_string()));
            if !matches!(v, Value::Nil) {
                return Some(v);
            }
        }
        None
    }

    pub fn by_meta_method(&self, method: MetaMethod) -> Result<Value<'v>, SiltError> {
        // println!("meta: {}", self.meta.clone().unwrap_or(Value::Nil));
        if let Some(Value::Table(t)) = &self.meta {
            // let s = method.as_table_key().to_string();
            // println!("looking for meta method: {}", s);
            if let Some(func) = t
                .borrow()
                .get(Value::String(method.as_table_key().to_string()))
            {
                // println!("found meta method: {}", func);
                return if let Value::Closure(_) = func {
                    Ok(func.clone())
                } else {
                    Err(SiltError::MetaMethodNotCallable(method))
                };
            }
        }
        Err(SiltError::MetaMethodMissing(method))
    }
    pub fn iter(&self) -> Iter<'_, Value<'v>, Value<'v>> {
        self.data.iter()
    }

    pub fn list_keys(&self)-> String{
        self.data.keys().map(|k| k.to_string()).collect::<Vec<String>>().join(",")
        
    }
}

impl ToString for Table<'_> {
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
