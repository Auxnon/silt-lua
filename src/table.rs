use std::{
    collections::{hash_map::Iter, HashMap},
    vec::IntoIter,
};

use gc_arena::{Collect, Mutation};

use crate::{
    error::SiltError,
    userdata::MetaMethod,
    value::{ExVal, Value},
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

    pub fn insert<'f>(&mut self, key: Value<'v>, value: Value<'v>) {
        self.data.insert(key, value);
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

    pub fn set<'f, K, V>(&mut self, key: K, val: V) -> Option<Value<'v>>
    where
        'v: 'f,
        K: Into<Value<'v>>,
        V: Into<Value<'v>>,
    {
        self.data.insert(key.into(), val.into())
    }

    pub fn to_array<'f, T, const N: usize>(&self) -> [T; N]
    where
        'v: 'f,
        T: Default,
        T: Copy,
        T: From<Value<'v>>,
    {
        let mut t = self.data.iter();
        let mut out: [T; N] = [T::default(); N];
        for i in 0..N {
            out[i] = if let Some(tt) = t.next() {
                T::from(tt.1.clone())
            } else {
                T::default()
            }
        }
        out
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
    pub fn push(&mut self, value: Value<'v>) {
        // DEV this just feels clunky to replicate lua's behavior
        self.counter += 1;
        let mut key = Value::Integer(self.counter);
        while self.data.contains_key(&key) {
            self.counter += 1;
        }
        key.force_to_int(self.counter);
        self.data.insert(key, value);
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
    pub fn iter(&self) -> Iter<'_, Value<'v>, Value<'v>> {
        self.data.iter()
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
