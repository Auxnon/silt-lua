use std::collections::{HashMap, HashSet};

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

/// Maximum size for the array part of a table before values go into the hash part
/// This limit helps prevent excessive memory allocation for sparse arrays
const MAX_ARRAY_SIZE: i64 = 1024;

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
        } else if i == (self.array.len() + 1) as i64 && i <= MAX_ARRAY_SIZE {
            // Allow extending array by one position, up to MAX_ARRAY_SIZE
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
                if let Some(idx) = table.array_index(i) {
                    // Use array_index helper for consistency
                    if idx >= table.array.len() {
                        table.array.resize(idx + 1, Value::Nil);
                    }
                    table.array[idx] = vv;
                } else {
                    table.hash.insert(kk, vv);
                }
            } else {
                table.hash.insert(kk, vv);
            }
        }
        table.counter = table.array.len() as i64;
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
                self.counter = self.array.len() as i64;
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
                self.counter = self.array.len() as i64;
                return if matches!(old, Value::Nil) { None } else { Some(old) };
            }
        }
        self.hash.insert(k, v)
    }

    pub fn to_exval(&self) -> ExTable {
        let mut visited = HashSet::new();
        self.to_exval_with_visited(&mut visited)
    }

    /// Convert table to ExTable with cycle detection using a DAG approach.
    /// If a cycle is detected (table references itself or creates a cycle),
    /// the cyclic reference will be replaced with a special marker ExVal::Meta("cyclic_ref").
    fn to_exval_with_visited(&self, visited: &mut HashSet<usize>) -> ExTable {
        let mut array = Vec::new();
        let mut hash = HashMap::new();
        
        // Mark this table as visited
        visited.insert(self.id);
        
        // Add array part - preserve all values including Nil to maintain indices
        for v in self.array.iter() {
            array.push(self.value_to_exval(v, visited));
        }
        
        // Add hash part
        for (k, v) in self.hash.iter() {
            hash.insert(
                self.value_to_exval(k, visited),
                self.value_to_exval(v, visited)
            );
        }
        
        // Unmark this table (allow it to appear in other branches of the tree)
        visited.remove(&self.id);
        
        ExTable {
            id: self.id,
            array,
            hash,
        }
    }

    /// Convert a Value to ExVal with cycle detection
    fn value_to_exval(&self, value: &Value<'v>, visited: &mut HashSet<usize>) -> ExVal {
        match value {
            Value::Table(t) => {
                let table_ref = t.borrow();
                let table_id = table_ref.id;
                
                // Check if we've already visited this table (cycle detected)
                if visited.contains(&table_id) {
                    // Return a marker for cyclic reference
                    ExVal::Meta(format!("cyclic_ref:table{}", table_id))
                } else {
                    // Recursively convert the table
                    ExVal::Table(table_ref.to_exval_with_visited(visited))
                }
            },
            // For all other value types, use the standard conversion
            _ => value.clone().into(),
        }
    }

    /// Returns the length of the array part (Lua # operator behavior).
    /// Note: This only counts the array part, not the total key-value pairs in the table.
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

/// Iterator over table entries. Iterates over array part first (1-indexed keys),
/// then hash part. Skips Value::Nil entries in the array part.
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
            // Skip nil entries (Lua semantics: nil means no value)
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
        let mut visited = HashSet::new();
        self.to_string_with_visited(&mut visited)
    }
}

impl Table<'_> {
    /// Convert table to string with cycle detection
    fn to_string_with_visited(&self, visited: &mut HashSet<usize>) -> String {
        // Check for cycles
        if visited.contains(&self.id) {
            return format!("table{}[cyclic]", self.id);
        }
        
        visited.insert(self.id);
        
        let mut entries = Vec::new();
        
        // Add array entries
        for (idx, v) in self.array.iter().enumerate() {
            if !matches!(v, Value::Nil) {
                let v_str = self.value_to_string(v, visited);
                entries.push(format!("{}: {}", idx + 1, v_str));
            }
        }
        
        // Add hash entries
        for (k, v) in self.hash.iter() {
            let k_str = self.value_to_string(k, visited);
            let v_str = self.value_to_string(v, visited);
            entries.push(format!("{}: {}", k_str, v_str));
        }
        
        visited.remove(&self.id);
        
        format!(
            "table{}[{}]{{{}}}",
            self.id,
            self.array.len() + self.hash.len(),
            entries.join(", ")
        )
    }
    
    /// Convert a value to string with cycle detection for nested tables
    fn value_to_string(&self, value: &Value, visited: &mut HashSet<usize>) -> String {
        match value {
            Value::Table(t) => {
                t.borrow().to_string_with_visited(visited)
            },
            _ => value.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExTable {
    id: usize,
    /** Array part: stores values indexed by integers starting from 1 (Lua convention) */
    array: Vec<ExVal>,
    /** Hash part: stores all non-integer keys and out-of-bounds integer keys */
    hash: HashMap<ExVal, ExVal>,
}

impl ExTable {
    /// Access array by 1-indexed integer
    pub fn getn(&self, i: usize) -> Option<&ExVal> {
        if i >= 1 && i <= self.array.len() {
            Some(&self.array[i - 1])
        } else {
            self.hash.get(&ExVal::Integer(i as i64))
        }
    }
    
    /// Remove from array by 1-indexed integer
    pub fn pop_value(&mut self, i: usize) -> ExVal {
        if i >= 1 && i <= self.array.len() {
            std::mem::replace(&mut self.array[i - 1], ExVal::Nil)
        } else {
            self.hash.remove(&ExVal::Integer(i as i64)).unwrap_or(ExVal::Nil)
        }
    }
    
    /// Access by string key from hash
    pub fn get(&self, field: &str) -> Option<&ExVal> {
        self.hash.get(&ExVal::String(field.to_owned()))
    }

}

impl PartialEq for ExTable {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ToString for ExTable {
    fn to_string(&self) -> String {
        let mut entries = Vec::new();
        
        // Add array entries (1-indexed)
        for (idx, v) in self.array.iter().enumerate() {
            if !matches!(v, ExVal::Nil) {
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
    type IntoIter = ExTableIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        ExTableIntoIter {
            array: self.array,
            array_index: 0,
            hash_iter: self.hash.into_iter(),
        }
    }
}

pub struct ExTableIntoIter {
    array: Vec<ExVal>,
    array_index: usize,
    hash_iter: std::collections::hash_map::IntoIter<ExVal, ExVal>,
}

impl Iterator for ExTableIntoIter {
    type Item = (ExVal, ExVal);

    fn next(&mut self) -> Option<Self::Item> {
        // First iterate over array part (1-indexed)
        while self.array_index < self.array.len() {
            let idx = self.array_index;
            self.array_index += 1;
            let val = &self.array[idx];
            // Skip nil entries
            if !matches!(val, ExVal::Nil) {
                return Some((ExVal::Integer((idx + 1) as i64), val.clone()));
            }
        }
        
        // Then iterate over hash part
        self.hash_iter.next()
    }
}

impl<'a> IntoIterator for &'a ExTable {
    type Item = (ExVal, &'a ExVal);
    type IntoIter = ExTableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ExTableIter {
            table: self,
            array_index: 0,
            hash_iter: self.hash.iter(),
        }
    }
}

pub struct ExTableIter<'a> {
    table: &'a ExTable,
    array_index: usize,
    hash_iter: std::collections::hash_map::Iter<'a, ExVal, ExVal>,
}

impl<'a> Iterator for ExTableIter<'a> {
    type Item = (ExVal, &'a ExVal);

    fn next(&mut self) -> Option<Self::Item> {
        // First iterate over array part (1-indexed)
        while self.array_index < self.table.array.len() {
            let idx = self.array_index;
            self.array_index += 1;
            let val = &self.table.array[idx];
            // Skip nil entries
            if !matches!(val, ExVal::Nil) {
                return Some((ExVal::Integer((idx + 1) as i64), val));
            }
        }
        
        // Then iterate over hash part
        self.hash_iter.next().map(|(k, v)| (k.clone(), v))
    }
}

impl<'a> IntoIterator for &'a mut ExTable {
    type Item = (ExVal, &'a mut ExVal);
    type IntoIter = ExTableIterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ExTableIterMut {
            array: &mut self.array,
            array_index: 0,
            hash_iter: self.hash.iter_mut(),
        }
    }
}

pub struct ExTableIterMut<'a> {
    array: &'a mut Vec<ExVal>,
    array_index: usize,
    hash_iter: std::collections::hash_map::IterMut<'a, ExVal, ExVal>,
}

impl<'a> Iterator for ExTableIterMut<'a> {
    type Item = (ExVal, &'a mut ExVal);

    fn next(&mut self) -> Option<Self::Item> {
        // First iterate over array part (1-indexed)
        while self.array_index < self.array.len() {
            let idx = self.array_index;
            self.array_index += 1;
            // Skip nil entries
            if !matches!(self.array[idx], ExVal::Nil) {
                // SAFETY: This is sound because:
                // 1. We increment array_index before returning, so the same element is never accessed twice
                // 2. The lifetime 'a is tied to the array reference, not to self
                // 3. We never create aliasing mutable references as we only return one per call
                let val_ptr = &mut self.array[idx] as *mut ExVal;
                unsafe {
                    return Some((ExVal::Integer((idx + 1) as i64), &mut *val_ptr));
                }
            }
        }
        
        // Then iterate over hash part
        self.hash_iter.next().map(|(k, v)| (k.clone(), v))
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
        (value.pop_value(1).into(), value.pop_value(2).into())
    }
}
