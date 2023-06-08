use std::collections::HashMap;

use crate::value::Value;

pub struct Environment {
    pub variables: HashMap<String, Value>,
    pub enclosing: Option<Box<Environment>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
            enclosing: None,
        }
    }
    pub fn set(&mut self, ident: String, value: Value) {
        self.variables.insert(ident, value);
    }
    pub fn get(&self, ident: &str) -> &Value {
        match self.variables.get(ident) {
            Some(v) => v.clone(),
            None => match &self.enclosing {
                Some(e) => e.get(ident),
                None => &Value::Nil,
            },
        }
    }
    pub fn create_enclosing(&mut self) {
        self.enclosing = Some(Box::new(Environment::new()));
    }
}
