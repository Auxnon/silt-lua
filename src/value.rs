use std::rc::Rc;

use hashbrown::HashMap;

use crate::{environment::Environment, error::ErrorTypes, function::FunctionObject};

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
    Table(Reference<HashMap<Value, Value>>),
    // Array // TODO lua 5 has an actual array type chosen contextually, how much faster can we make a table by using it?
    // Boxed()
    Function(Rc<FunctionObject>), // closure: Environment,

    // Func(fn(Vec<Value>) -> Value)
    NativeFunction(fn(&mut Environment, Vec<Value>) -> Value),
    // UserData
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
            Value::Integer(i) => write!(f, "{}i", i),
            Value::Number(n) => write!(f, "{}f", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Infinity(_) => write!(f, "inf"),
            Value::NativeFunction(_) => write!(f, "native_function"),
            Value::Function(ff) => {
                if ff.is_script {
                    write!(
                        f,
                        "module {}",
                        ff.name.as_ref().unwrap_or(&"root".to_string())
                    )
                } else {
                    write!(
                        f,
                        "fn {}()",
                        ff.name.as_ref().unwrap_or(&"anonymous".to_string())
                    )
                }
            }
            Value::Table(t) => write!(f, "table: {}", t.id),
        }
    }
}

impl core::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Value {
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
            Value::Table(_) => ErrorTypes::Table,
        }
    }

    fn to_string(&self) -> String {
        match self {
            Value::Integer(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Nil => "nil".to_string(),
            Value::Infinity(b) => {
                if *b {
                    "-ing".to_string()
                } else {
                    "inf".to_string()
                }
            }
            Value::NativeFunction(_) => "native_function".to_string(),
            Value::Function { .. } => "function".to_string(),
            Value::String(s) => s.to_string(),
            Value::Table(t) => format!("table: {}", t.id),
        }
    }
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
            Value::NativeFunction(f) => Value::NativeFunction(*f),
            // TODO: implement this
            Value::Function(r) => Value::Function(Rc::clone(r)),
            Value::Table(t) => Value::Table(Reference {
                value: Rc::clone(&t.value),
                id: t.id,
            }),
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
                i as *const fn(&mut Environment, Vec<Value>) -> Value
                    == j as *const fn(&mut Environment, Vec<Value>) -> Value
            }
            (Value::Function(i), Value::Function(j)) => Rc::ptr_eq(i, j),
            (Value::Table(i), Value::Table(j)) => Rc::ptr_eq(&i.value, &j.value),
            _ => false,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Nil
    }
}
