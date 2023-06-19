use std::rc::Rc;

use crate::{environment::Environment, error::ErrorTypes, function::ScopedFunction};

pub enum Value {
    Integer(i64),
    Number(f64),
    Bool(bool),
    Nil,
    /** true for negative */
    Infinity(bool),
    // Bool(bool),
    String(Box<str>),
    // List(Vec<Value>),
    // Map(HashMap<String, Value>),
    Function(Rc<ScopedFunction>), // closure: Environment,

    // Func(fn(Vec<Value>) -> Value)
    NativeFunction(fn(&mut Environment, Vec<Value>) -> Value),
    // UserData
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
            Value::Function { .. } => write!(f, "function"),
        }
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
        }
    }
}
