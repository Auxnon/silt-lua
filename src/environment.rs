use std::collections::HashMap;

use crate::value::Value;

pub struct Environment {
    pub variables: Vec<HashMap<String, Value>>,
    depth: usize, // pub enclosing: Option<&'b mut Environment<'a, 'b>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: vec![HashMap::new()],
            depth: 0, // enclosing: None,
        }
    }
    pub fn set(&mut self, ident: String, value: Value, declare: bool) {
        if declare {
            self.variables[self.depth].insert(ident, value);
        } else {
            if self.depth > 0 {
                for vars in self.variables.iter_mut().rev() {
                    if let Some(val) = vars.get_mut(&ident) {
                        *val = value;
                        return;
                    }
                }
            }
            self.variables[0].insert(ident, value);
        }
    }
    pub fn get(&self, ident: &str) -> &Value {
        // match self.variables.get(ident) {
        //     Some(v) => v.clone(),
        //     None => match &self.enclosing {
        //         Some(e) => e.get(ident),
        //         None => &Value::Nil,
        //     },
        // }
        if self.depth > 0 {
            for vars in self.variables.iter().rev() {
                if let Some(val) = vars.get(ident) {
                    return val;
                }
            }
        } else {
            if let Some(val) = self.variables[0].get(ident) {
                return val;
            }
        }
        // vars.get(ident)
        &Value::Nil
    }
    pub fn new_scope(&mut self) {
        self.variables.push(HashMap::new());
        self.depth += 1;
    }
    pub fn pop_scope(&mut self) {
        self.variables.pop();
        self.depth -= 1;
    }
    // pub fn create_enclosing(&mut self, parent: &'a mut Environment<'a, 'b>) {
    //     self.enclosing = Some(parent);
    // }
}
