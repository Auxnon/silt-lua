use crate::error::ErrorTypes;

pub enum Value {
    Integer(i64),
    Number(f64),
    Bool(bool),
    Nil,
    /** true for negative */
    Infinity(bool),
    // Bool(bool),
    String(String),
    // List(Vec<Value>),
    // Map(HashMap<String, Value>),
    // Func(fn(Vec<Value>) -> Value),
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
        }
    }
}
