use hashbrown::HashMap;

use crate::value::Value;

pub struct Table {
    data: HashMap<Value, Value>,
    /** replicate standard lua behavior */
    counter: i64,
    id: usize,
}

impl Table {
    pub fn new(id: usize) -> Self {
        Table {
            data: HashMap::new(),
            counter: 0,
            id,
        }
    }
    pub fn insert(&mut self, key: Value, value: Value) {
        self.data.insert(key, value);
    }
    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.data.get(key)
    }
    pub fn getn(&self, i: usize) -> Option<&Value> {
        self.data.get(&Value::Integer(i as i64))
    }
    pub fn get_value(&self, key: &Value) -> Value {
        match self.data.get(key) {
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    /** push by counter's current index, if it aready exists keep incrementing until empty position is found */
    pub fn push(&mut self, value: Value) {
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

impl ToString for Table {
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
