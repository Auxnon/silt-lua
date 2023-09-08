use std::{cell::RefCell, rc::Rc};

use hashbrown::HashMap;

use crate::{
    error::ErrorTypes,
    function::{Closure, FunctionObject, NativeObject},
    lua::Lua,
    table::Table,
    userdata::UserData,
};

/** Lua value enum representing different data types within a VM */
pub enum Value {
    Nil,
    Integer(i64),
    Number(f64),
    Bool(bool),
    /** true for negative */
    Infinity(bool),
    // Bool(bool),
    // TODO in most cases strings are just copied around on the heap, this is expensive but saves us from using GC here
    // TODO 2 consider other encoding options for efficiency. Is having a seperate ASCII string type beneficial to only english speakers? how would other speaking languages handle ascii strings without needed glyphs?
    String(Box<String>),
    // List(Vec<Value>),
    // Map(HashMap<String, Value>),
    Table(Rc<RefCell<Table>>),
    // Array // TODO lua 5 has an actual array type chosen contextually, how much faster can we make a table by using it?
    // Boxed()
    Function(Rc<FunctionObject>), // closure: Environment,
    Closure(Rc<Closure>),
    // Func(fn(Vec<Value>) -> Value)
    NativeFunction(Rc<NativeObject>),
    UserData(Rc<dyn UserData>),
}

pub enum ReferneceStore {
    Table(HashMap<Value, Value>),
}
pub struct Reference<T> {
    pub value: Rc<T>,
    pub id: usize,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{}", i),
            Value::Number(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Infinity(_) => write!(f, "inf"),
            Value::NativeFunction(_) => write!(f, "native_function"),
            Value::Closure(c) => write!(f, "(clojure= {})", c.function),
            Value::Function(ff) => write!(f, "{}", ff),
            Value::Table(t) => write!(f, "{}", t.borrow().to_string()),
            Value::UserData(u) => write!(f, "{}", u.to_string()),
        }
    }
}

impl core::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Value {
    /** Condense value into a tiny enum for passing to errors*/
    pub fn to_error(&self) -> ErrorTypes {
        match self {
            Value::Integer(_) => ErrorTypes::Integer,
            Value::Number(_) => ErrorTypes::Number,
            Value::Bool(_) => ErrorTypes::Bool,
            Value::Nil => ErrorTypes::Nil,
            Value::String(_) => ErrorTypes::String,
            Value::Infinity(_) => ErrorTypes::Infinity,
            Value::NativeFunction(_) => ErrorTypes::NativeFunction,
            Value::Function { .. } => ErrorTypes::Function,
            Value::Closure(_) => ErrorTypes::Closure,
            Value::Table(_) => ErrorTypes::Table,
            Value::UserData(_) => ErrorTypes::UserData,
        }
    }

    pub fn force_to_int(&mut self, n: i64) {
        *self = Value::Integer(n);
    }

    // fn to_string(&self) -> String {
    //     match self {
    //         Value::Integer(i) => i.to_string(),
    //         Value::Number(n) => n.to_string(),
    //         Value::Bool(b) => b.to_string(),
    //         Value::Nil => "nil".to_string(),
    //         Value::Infinity(b) => {
    //             if *b {
    //                 "-ing".to_string()
    //             } else {
    //                 "inf".to_string()
    //             }
    //         }
    //         Value::NativeFunction(_) => "native_function".to_string(),
    //         Value::Function { .. } => "function".to_string(),
    //         Value::Closure(_) => "(function)".to_string(),
    //         Value::String(s) => s.to_string(),
    //         Value::Table(t) => t.borrow().to_string(),
    //         Value::UserData(u) => u.to_string(),
    //     }
    // }
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Value::Integer(i) => Value::Integer(*i),
            Value::Number(n) => Value::Number(*n),
            Value::Bool(b) => Value::Bool(*b),
            Value::Nil => Value::Nil,
            Value::String(s) => Value::String(s.clone()),
            Value::Infinity(b) => Value::Infinity(*b),
            Value::NativeFunction(f) => Value::NativeFunction(f.clone()),
            // TODO: implement this
            Value::Function(r) => Value::Function(Rc::clone(r)),
            Value::Closure(c) => Value::Closure(Rc::clone(c)),
            // Value::Table(t) => Value::Table(Reference {
            //     value: Rc::clone(&t.value),
            //     id: t.id,
            // }),
            Value::Table(t) => Value::Table(Rc::clone(t)),
            Value::UserData(u) => Value::UserData(Rc::clone(u)),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(i), Value::Integer(j)) => i == j,
            (Value::Number(i), Value::Number(j)) => i == j,
            (Value::Bool(i), Value::Bool(j)) => i == j,
            (Value::Nil, Value::Nil) => true,
            (Value::String(i), Value::String(j)) => i == j,
            (Value::Infinity(i), Value::Infinity(j)) => i == j,
            (Value::NativeFunction(i), Value::NativeFunction(j)) => {
                i.function as *const fn(&mut Lua, Vec<Value>) -> Value
                    == j.function as *const fn(&mut Lua, Vec<Value>) -> Value
            }
            (Value::Function(i), Value::Function(j)) => Rc::ptr_eq(i, j),
            (Value::Table(i), Value::Table(j)) => Rc::ptr_eq(&i, &j),
            _ => false,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Nil
    }
}

impl core::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl Eq for Value {}

// impl Drop for Value {
//     fn drop(&mut self) {
//         println!("dropping value: {}", self);
//     }
// }
