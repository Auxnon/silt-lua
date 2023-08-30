use hashbrown::HashMap;

use crate::value::Value;

pub struct Table {
    data: HashMap<Value, Value>,

    id: usize,
}

impl Table {
    pub fn new(id: usize) -> Self {
        Table {
            data: HashMap::new(),
            id,
        }
    }
    pub fn insert(&mut self, key: Value, value: Value) {
        self.data.insert(key, value);
    }
    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.data.get(key)
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
    pub fn push(&mut self, value: Value) {
        self.data
            .insert(Value::Integer(self.data.len() as i64), value);
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
