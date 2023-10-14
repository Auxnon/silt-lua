use hashbrown::HashMap;

use crate::value::{ExVal, Value};

pub struct Table<'v> {
    data: HashMap<Value<'v>, Value<'v>>,
    /** replicate standard lua behavior */
    counter: i64,
    id: usize,
}

impl<'v> Table<'v> {
    pub fn new(id: usize) -> Self {
        Table {
            data: HashMap::new(),
            counter: 0,
            id,
        }
    }

    pub fn insert<'f>(&mut self, key: Value<'v>, value: Value<'v>) {
        self.data.insert(key, value);
    }

    pub fn get<'f>(&'v self, key: &Value<'f>) -> Option<&Value<'f>>
    where
        'v: 'f,
    {
        self.data.get(key)
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
