// use rustc_hash::FxHashMap as HashMap;
use hashbrown::HashMap;
use std::vec;
// use std::println;

use crate::value::Value;

pub struct Environment {
    pub variables: Vec<HashMap<usize, Value>>,
    depth: usize, // pub enclosing: Option<&'b mut Environment<'a, 'b>>,
    /** Whether undeclared variables should implicitly define up to the top level like normal lua, or start in immediate scope */
    implicit_global: bool,
    strict: bool,
    free_registers: Vec<usize>,
    next_register: usize,
    map: HashMap<String, usize>,
    // meta_table: HashMap<usize, usize>, // key, met
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: vec![HashMap::default()],
            depth: 0, // enclosing: None,
            implicit_global: true,
            strict: false,
            free_registers: vec![],
            next_register: 0,
            map: HashMap::default(),
        }
    }
    
    pub fn new_with_std() -> Self {
        let mut env = Environment::new();
        env.load_standard_library();
        env
    }

    pub fn load_standard_library(&mut self) {
        let u = self.to_register("clock");
        self.set(
            u,
            Value::NativeFunction(crate::standard::clock),
            true,
            false,
        );
    }

    pub fn declare_local(&mut self, ident: usize, value: Value) {
        self.set(ident, value, true, true);
    }
    pub fn assign_local(&mut self, ident: usize, value: Value) {
        self.set(ident, value, false, true);
    }

    /** If we have implicit global then we default implicit declaration to the highest level as lua
     * does, otherwise we do something nicer and create in local scope if not shadowing anything*/
    pub fn set(&mut self, ident: usize, value: Value, declare: bool, local: bool) {
        if declare {
            // println!(" declare {} at {}", ident, self.depth);
            self.variables[if local { self.depth } else { 0 }].insert(ident, value);
        } else {
            if self.depth > 0 {
                for vars in self.variables.iter_mut().rev() {
                    if let Some(val) = vars.get_mut(&ident) {
                        *val = value;
                        return;
                    }
                }
            }
            if self.implicit_global {
                self.variables[0].insert(ident, value);
            } else {
                self.variables[self.depth].insert(ident, value);
            }
        }
    }
    pub fn get(&self, ident: &usize) -> &Value {
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
                // println!("got");
                return val;
            }
        }
        // vars.get(ident)
        &Value::Nil
    }
    pub fn new_scope(&mut self) {
        self.variables.push(HashMap::default());
        self.depth += 1;
    }
    pub fn pop_scope(&mut self) {
        self.variables.pop();
        self.depth -= 1;
    }
    fn get_register(&mut self) -> usize {
        if let Some(reg) = self.free_registers.pop() {
            reg
        } else {
            let reg = self.next_register;
            self.next_register += 1;
            reg
        }
    }

    fn free_register(&mut self, reg: usize) {
        self.free_registers.push(reg);
    }

    pub fn to_register(&mut self, name: &str) -> usize {
        if let Some(reg) = self.map.get(name) {
            *reg
        } else {
            let reg = self.get_register();
            // println!("to_register {} @ {}", name, reg);
            self.map.insert(name.to_string(), reg);
            reg
        }
    }
    pub fn is_strict(&self) -> bool {
        self.strict
    }
    // pub fn create_enclosing(&mut self, parent: &'a mut Environment<'a, 'b>) {
    //     self.enclosing = Some(parent);
    // }
}
