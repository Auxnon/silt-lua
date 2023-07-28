// use rustc_hash::FxHashMap as HashMap;
// use hashbrown::HashMap;
use std::{cell::RefCell, collections::HashMap, rc::Rc, vec};
// use std::println;

use crate::{expression::Ident, value::Value};

pub type Scope = Rc<RefCell<HashMap<usize, Value>>>;

pub struct Environment {
    pub variables: Vec<Scope>,
    depth: usize, // pub enclosing: Option<&'b mut Environment<'a, 'b>>,
    /** Whether undeclared variables should implicitly define up to the top level like normal lua, or start in immediate scope */
    implicit_global: bool,
    strict: bool,
    free_registers: Vec<usize>,
    next_register: usize,
    map: HashMap<String, usize>,
    // local_table: HashMap<usize, usize>,
    // meta_table: HashMap<usize, usize>, // key
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            variables: vec![Rc::new(RefCell::new(HashMap::default()))],
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
        self.register_std("clock", crate::standard::clock);
        self.register_std("print", crate::standard::print);
    }

    fn register_std(
        &mut self,
        name: &str,
        func: fn(&mut Environment, Vec<Value>) -> Value,
    ) -> usize {
        let u = self.to_register(name);
        self.declare_global((u, 0), Value::NativeFunction(func));
        u
    }

    pub fn declare_global(&mut self, ident: Ident, value: Value) {
        self.set(ident, value, true, false);
    }

    pub fn declare_local(&mut self, ident: Ident, value: Value) {
        self.set(ident, value, true, true);
    }
    pub fn set_in_scope(&mut self, ident: usize, value: Value) {
        self.variables[self.depth].borrow_mut().insert(ident, value);
    }

    pub fn assign_local(&mut self, ident: Ident, value: Value) {
        self.set(ident, value, false, true);
    }

    /** If we have implicit global then we default implicit declaration to the highest level as lua
     * does, otherwise we do something nicer and create in local scope if not shadowing anything*/
    pub fn set(&mut self, ident: Ident, value: Value, declare: bool, local: bool) {
        // if declare {
        //     self.variables[if local { self.depth } else { 0 }].insert(ident, value);
        // } else {
        //     // println!(
        //     //     "assign {} to {} (current depth {})",
        //     //     value, ident, self.depth
        //     // );
        //     // check if exists
        //     if self.depth > 0 {
        //         // for vars in self.variables.iter_mut().rev() {
        //         //     if let Some(val) = vars.get_mut(&ident) {
        //         //         *val = value;
        //         //         return;
        //         //     }
        //         // }
        //         for i in (0..=self.depth).rev() {
        //             if let Some(val) = self.variables[i].get_mut(&ident) {
        //                 *val = value;
        //                 return;
        //             }
        //         }
        //     }
        //     // implicit declaration as we were unable to find an existing variable
        //     if self.implicit_global {
        //         self.variables[0].insert(ident, value);
        //     } else {
        //         self.variables[self.depth].insert(ident, value);
        //     }
        // }

        self.variables[ident.1].borrow_mut().insert(ident.0, value);
    }

    pub fn get(&self, ident: &Ident) -> Value {
        // match self.variables.get(ident) {
        //     Some(v) => v.clone(),
        //     None => match &self.enclosing {
        //         Some(e) => e.get(ident),
        //         None => &Value::Nil,
        //     },
        // }
        if let Some(val) = self.variables[ident.1].borrow().get(&ident.0) {
            val.clone()
        } else {
            Value::Nil
        }

        // if self.depth > 0 {
        //     for vars in self.variables.iter().rev() {
        //         if let Some(val) = vars.get(ident) {
        //             return val.clone();
        //         }
        //     }
        // } else {
        //     if let Some(val) = self.variables[0].get(ident) {
        //         // println!("got");
        //         return val.clone();
        //     }
        // }
        // vars.get(ident)
    }

    // pub fn get_at(&self, ident: &usize, depth: usize) -> Value {
    //     if let Some(val) = self.variables[depth].borrow().get(ident) {
    //         return val.clone();
    //     }
    //     Value::Nil
    // }

    pub fn new_scope(&mut self) {
        self.variables
            .push(Rc::new(RefCell::new(HashMap::default())));
        self.depth += 1;
    }

    pub fn pop_scope(&mut self) {
        // self.variables.get_mut(self.depth).unwrap().clear(); // not faster, +2-3% time, despite keeping memory
        // self.variables[self.depth].clear(); // about the same as popping +1% time, but keeps memory
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

    pub fn get_current_scope(&self) -> Vec<Scope> {
        self.variables
            .iter()
            .rev()
            .take(self.depth + 1)
            .map(|s| s.clone())
            .collect()
    }

    ///////////////////////////
    pub fn swap_scope(&mut self, scope: &Vec<Scope>) -> Vec<Scope> {
        let temp_scope = self.variables.drain(0..=self.depth).collect::<Vec<_>>();
        self.variables.extend(scope.iter().rev().cloned());
        self.depth = self.variables.len() - 1;
        temp_scope
    }
    pub fn replace_scope(&mut self, scope: Vec<Scope>) {
        self.variables = scope;
        self.depth = self.variables.len() - 1;
    }
    ///////////////////////////

    pub fn resolve_local(&mut self, ident: usize, depth: usize) {
        // TODO use the expression as a key? the expr as key should point to a the depth
        // self.local_table.insert(ident, depth);
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
    pub fn from_register(&self, reg: usize) -> &str {
        for (k, v) in self.map.iter() {
            if *v == reg {
                return k;
            }
        }
        "unknown"
    }

    pub fn is_strict(&self) -> bool {
        self.strict
    }
}
