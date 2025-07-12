use std::collections::HashMap;

use gc_arena::Collect;

use crate::{
    error::SiltError,
    userdata::MetaMethod,
    value::{ExVal, Value},
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
    // pub fn display(&self){
    //     self.data.

    /** push by counter's current index, if it aready exists keep incrementing until empty position is found */
    pub fn push(&mut self, value: Value<'v>) {
        // DEV this just feels clunky to replicate lua's behavior
        self.counter += 1;
        let mut key = Value::Integer(self.counter);
        while self.data.contains_key(&key) {
            self.counter += 1;
            key.force_to_int(self.counter);
        }
        self.data.insert(key, value);
    }

    pub fn set_metatable(&mut self, metatable: Value<'v>) {
        println!("setting metatable: {}", metatable);
        self.meta = Some(metatable);
    }

    pub fn get_metatable(&self) -> Value<'v> {
        self.meta.clone().unwrap_or(Value::Nil)
    }

    pub fn by_meta_method(&self, method: MetaMethod) -> Result<Value<'v>, SiltError> {
        println!("meta: {}", self.meta.clone().unwrap_or(Value::Nil));
        if let Some(Value::Table(t)) = &self.meta {
            let s = method.as_table_key().to_string();
            println!("looking for meta method: {}", s);
            if let Some(func) = t
                .borrow()
                .get(Value::String(method.as_table_key().to_string()))
            {
                println!("found meta method: {}", func);
                return if let Value::Closure(_) = func {
                    Ok(func.clone())
                } else {
                    Err(SiltError::MetaMethodNotCallable(method))
                };
            }
        }
        return Err(SiltError::MetaMethodMissing(method));
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
    pub fn get(&self, field: &str) -> Option<&ExVal> {
        self.data.get(&ExVal::String(field.to_owned()))
    }
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
