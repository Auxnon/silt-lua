use std::collections::HashMap;

use gc_arena::{Collect, Mutation};

use crate::{
    error::SiltError,
    userdata::MetaMethod,
    value::{ExVal,  Value},
    VM,
};

#[derive(Collect)]
#[collect(no_drop)]
pub struct Table<'v> {
    /** Array part: stores values indexed by integers starting from 1 (Lua convention) */
    array: Vec<Value<'v>>,
    /** Hash part: stores all non-integer keys and out-of-bounds integer keys */
    hash: HashMap<Value<'v>, Value<'v>>,
    meta: Option<Value<'v>>,
    // data: RefLock<HashMap<String, String>>,
    /** replicate standard lua behavior */
    counter: i64,
    id: usize,
}

impl<'v> Table<'v> {
    pub fn new(id: usize) -> Self {
        Table {
            array: Vec::new(),
            hash: HashMap::new(),
            meta: None,
            counter: 0,
            id,
        }
    }

    /// Helper: determines if an integer key should use the array part
    /// Returns Some(index) if it should use array (0-based), None if it should use hash
    fn array_index(&self, i: i64) -> Option<usize> {
        if i >= 1 && i <= self.array.len() as i64 {
            Some((i - 1) as usize)
        } else if i == (self.array.len() + 1) as i64 && i <= 1024 {
            // Allow extending array up to a reasonable size
            Some(i as usize - 1)
        } else {
            None
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
        let mut table = Table::new(id);
        for (k, v) in input.into_iter() {
            let kk: Value = k.into_value(vm, mc)?;
            let vv = v.into_value(vm, mc)?;
            
            // Route to array or hash based on key type
            if let Value::Integer(i) = kk {
                if i >= 1 && i <= 1024 {
                    // Extend array if needed
                    let idx = (i - 1) as usize;
                    if idx >= table.array.len() {
                        table.array.resize(idx + 1, Value::Nil);
                    }
                    table.array[idx] = vv;
                    table.counter = table.counter.max(i);
                } else {
                    table.hash.insert(kk, vv);
                }
            } else {
                table.hash.insert(kk, vv);
            }
        }
        Ok(table)
    }

    pub fn insert<'f>(&mut self, key: Value<'v>, value: Value<'v>) {
        if let Value::Integer(i) = key {
            if let Some(idx) = self.array_index(i) {
                // Extend array if needed
                if idx >= self.array.len() {
                    self.array.resize(idx + 1, Value::Nil);
                }
                self.array[idx] = value;
                self.counter = self.counter.max(i);
                return;
            }
        }
        self.hash.insert(key, value);
    }

    // same as get but accepts reference Into<&Value> which is better
    pub fn getr<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<&'f Value<'v>>,
    {
        let k = key.into();
        if let Value::Integer(i) = k {
            if let Some(idx) = self.array_index(*i) {
                if idx < self.array.len() {
                    let val = &self.array[idx];
                    if !matches!(val, Value::Nil) {
                        return Some(val);
                    }
                }
            }
        }
        self.hash.get(k)
    }

    pub fn get<'f, T>(&self, key: T) -> Option<&Value<'v>>
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        let k = key.into();
        if let Value::Integer(i) = k {
            if let Some(idx) = self.array_index(i) {
                if idx < self.array.len() {
                    let val = &self.array[idx];
                    if !matches!(val, Value::Nil) {
                        return Some(val);
                    }
                }
            }
        }
        self.hash.get(&k)
    }

    pub fn getn(&self, i: usize) -> Option<&Value<'v>> {
        let idx_i64 = i as i64;
        if let Some(idx) = self.array_index(idx_i64) {
            if idx < self.array.len() {
                let val = &self.array[idx];
                if !matches!(val, Value::Nil) {
                    return Some(val);
                }
            }
        }
        self.hash.get(&Value::Integer(idx_i64))
    }

    pub fn get_value(&self, key: &Value<'v>) -> Value<'v> {
        if let Value::Integer(i) = key {
            if let Some(idx) = self.array_index(*i) {
                if idx < self.array.len() {
                    return self.array[idx].clone();
                }
            }
        }
        self.hash.get(key).cloned().unwrap_or(Value::Nil)
    }

    pub fn get_number<'f, T>(&self, key: T) -> f64
    where
        'v: 'f,
        T: Into<Value<'v>>,
    {
        let k = key.into();
        if let Value::Integer(i) = k {
            if let Some(idx) = self.array_index(i) {
                if idx < self.array.len() {
                    return (&self.array[idx]).into();
                }
            }
        }
        match self.hash.get(&k) {
            Some(v) => v.into(),
            _ => 0.,
        }
    }

    pub fn set<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        let k = key.into();
        let v = val.into();
        
        if let Value::Integer(i) = k {
            if let Some(idx) = self.array_index(i) {
                // Extend array if needed
                if idx >= self.array.len() {
                    self.array.resize(idx + 1, Value::Nil);
                }
                let old = std::mem::replace(&mut self.array[idx], v);
                self.counter = self.counter.max(i);
                return if matches!(old, Value::Nil) { None } else { Some(old) };
            }
        }
        self.hash.insert(k, v)
    }

    pub fn to_exval(&self) -> ExTable {
        let mut map = HashMap::new();
        
        // Add array part (1-indexed)
        for (idx, v) in self.array.iter().enumerate() {
            if !matches!(v, Value::Nil) {
                let key = ExVal::Integer((idx + 1) as i64);
                map.insert(key, v.clone().into());
            }
        }
        
        // Add hash part
        for (k, v) in self.hash.iter() {
            map.insert(k.clone().into(), v.clone().into());
        }
        
        ExTable {
            id: self.id,
            data: map,
        }
    }

    pub fn len(&self) -> usize {
        self.array.len()
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty() && self.hash.is_empty()
    }

    // pub fn display(&self){
    //     self.data.

    /** push by counter's current index, if it aready exists keep incrementing until empty position is found */
    pub fn push(&mut self, value: Value<'v>) {
        self.array.push(value);
        self.counter = self.array.len() as i64;
    }

    pub fn concat_array<A, I>(&mut self, array: I)
    where
        I: IntoIterator<Item = A>,
        A: Into<Value<'v>>,
    {
        for v in array.into_iter() {
            self.array.push(A::into(v));
        }
        self.counter = self.array.len() as i64;
    }

    pub fn set_metatable(&mut self, metatable: Value<'v>) {
        // println!("setting metatable: {}", metatable);
        self.meta = Some(metatable);
    }

    pub fn get_metatable(&self) -> Value<'v> {
        self.meta.clone().unwrap_or(Value::Nil)
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
    pub fn iter(&self) -> TableIterator<'_, 'v> {
        TableIterator {
            table: self,
            array_index: 0,
            hash_iter: self.hash.iter(),
        }
    }
}

pub struct TableIterator<'t, 'v> {
    table: &'t Table<'v>,
    array_index: usize,
    hash_iter: std::collections::hash_map::Iter<'t, Value<'v>, Value<'v>>,
}

impl<'t, 'v> Iterator for TableIterator<'t, 'v> {
    type Item = (Value<'v>, &'t Value<'v>);

    fn next(&mut self) -> Option<Self::Item> {
        // First iterate over array part
        while self.array_index < self.table.array.len() {
            let idx = self.array_index;
            self.array_index += 1;
            let val = &self.table.array[idx];
            if !matches!(val, Value::Nil) {
                return Some((Value::Integer((idx + 1) as i64), val));
            }
        }
        
        // Then iterate over hash part
        self.hash_iter.next().map(|(k, v)| (k.clone(), v))
    }
}

impl ToString for Table<'_> {
    fn to_string(&self) -> String {
        let mut entries = Vec::new();
        
        // Add array entries
        for (idx, v) in self.array.iter().enumerate() {
            if !matches!(v, Value::Nil) {
                entries.push(format!("{}: {}", idx + 1, v));
            }
        }
        
        // Add hash entries
        for (k, v) in self.hash.iter() {
            entries.push(format!("{}: {}", k, v));
        }
        
        format!(
            "table{}[{}]{{{}}}",
            self.id,
            self.array.len() + self.hash.len(),
            entries.join(", ")
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
    fn from( value: ExVal) -> Self {
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
    fn from( value: &mut ExVal) -> Self {
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
