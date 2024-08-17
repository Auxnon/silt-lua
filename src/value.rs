use std::{
    cell::RefCell,
    hash::{Hash, Hasher},
    rc::Rc,
};

use gc_arena::{lock::RefLock, Gc};
use hashbrown::HashMap;

#[cfg(feature = "vectors")]
use crate::vec::{Vec2, Vec3};
use crate::{
    error::{SiltError, ValueTypes},
    function::{Closure, FunctionObject, NativeFunction},
    lua::Lua,
    table::Table,
    token::Operator,
    userdata::{MetaMethod, UserData},
};

/**
 * Increment self by value and operand type, if increment op fails then use fallback, for instance += would fallback to +
 * The fallback is used in scenarios where trying to adjust self would change the type, such as an integer to float
 */
macro_rules! binary_self_op {
    ($l:ident, $op:tt, $fallback:tt, $r:ident, $opp:tt) => {
        {
            match(&mut *$l, $r){
                (Value::Number(left), Value::Number(right))  => *left $op right,
                (Value::Integer(left), Value::Integer(right)) =>*left $op right,
                (Value::Number(left), Value::Integer(right)) => *left $op (*right as f64),
                // (Value::Integer(left), Value::Number(right)) => Some(Value::Number((*left as f64) $fallback right)),
                (Value::Integer(left), Value::Number(right)) =>  *$l= Value::Number((*left as f64) $fallback right),

                #[cfg(feature = "vectors")]
                (Value::Vec3(left), Value::Vec3(right)) => *left $op *right,
                #[cfg(feature = "vectors")]
                (Value::Vec2(left), Value::Vec2(right)) => *left $op *right,

                // TODO
                (ll,rr) => return Err(SiltError::ExpOpValueWithValue(ll.to_error(), MetaMethod::$opp, rr.to_error()))
            }
            Ok(())
        }
    };
}

/** Lua value enum representing different data types within a VM */
pub enum Value<'v> {
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
    Table(Gc<'gc, RefLock<Table<'v>>>),
    // Array // TODO lua 5 has an actual array type chosen contextually, how much faster can we make a table by using it?
    // Boxed()
    Function(Gc<FunctionObject<'v>>), // closure: Environment,
    Closure(Gc<Closure<'v>>),
    // Func(fn(Vec<Value>) -> Value)
    NativeFunction(NativeFunction<'v>),
    UserData(Rc<dyn UserData<'v>>),
    #[cfg(feature = "vectors")]
    Vec3(Vec3),
    #[cfg(feature = "vectors")]
    Vec2(Vec2),
}

#[derive(Debug, Clone)]
pub enum ExVal {
    Nil,
    Integer(i64),
    Number(f64),
    Bool(bool),
    Infinity(bool),
    String(Box<String>),
    Table(crate::table::ExTable),
    Meta(Box<String>),
    // UserData(Rc<dyn UserData>),
    #[cfg(feature = "vectors")]
    Vec3(Vec3),
    #[cfg(feature = "vectors")]
    Vec2(Vec2),
}

impl Eq for ExVal {}
impl PartialEq for ExVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ExVal::Integer(i), ExVal::Integer(j)) => i == j,
            (ExVal::Number(i), ExVal::Number(j)) => i == j,
            (ExVal::Bool(i), ExVal::Bool(j)) => i == j,
            (ExVal::Nil, ExVal::Nil) => true,
            (ExVal::String(i), ExVal::String(j)) => i == j,
            (ExVal::Infinity(i), ExVal::Infinity(j)) => i == j,
            (ExVal::Table(i), ExVal::Table(j)) => i == j,
            _ => false,
        }
    }
}

impl Into<ExVal> for Value<'_> {
    fn into(self) -> ExVal {
        match self {
            Value::Nil => ExVal::Nil,
            Value::Integer(i) => ExVal::Integer(i),
            Value::Number(n) => ExVal::Number(n),
            Value::Bool(b) => ExVal::Bool(b),
            Value::Infinity(b) => ExVal::Infinity(b),
            Value::String(s) => ExVal::String(s),
            Value::Table(t) => ExVal::Table(t.borrow().to_exval()),
            Value::Function(f) => ExVal::Meta(format!("{}", f).into()),
            Value::Closure(c) => ExVal::Meta(format!("=>({})", c.function).into()),
            Value::NativeFunction(f) => ExVal::Meta(Box::new("native_function".to_string())),
            Value::UserData(u) => ExVal::Nil,
            #[cfg(feature = "vectors")]
            Value::Vec3(v) => ExVal::Vec3(v),
            #[cfg(feature = "vectors")]
            Value::Vec2(v) => ExVal::Vec2(v),
        }
    }
}

impl Hash for ExVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl std::fmt::Display for ExVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExVal::Integer(i) => write!(f, "{}", i),
            ExVal::Number(n) => write!(f, "{}", n),
            ExVal::Bool(b) => write!(f, "{}", b),
            ExVal::Nil => write!(f, "nil"),
            ExVal::String(s) | ExVal::Meta(s) => write!(f, "\"{}\"", s),
            ExVal::Infinity(b) => write!(f, "{}inf", if *b { "-" } else { "" }),
            ExVal::Table(t) => write!(f, "{}", t.to_string()),
            ExVal::Vec3(v) => write!(f, "{}", v),
            ExVal::Vec2(v) => write!(f, "{}", v),
        }
    }
}

// pub enum ReferneceStore {
//     Table(HashMap<Value, Value>),
// }
pub struct Reference<T> {
    pub value: Rc<T>,
    pub id: usize,
}

impl std::fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Integer(i) => write!(f, "{}", i),
            Value::Number(n) => write!(f, "{}", n),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
            Value::String(s) => write!(f, "\"{}\"", s),
            Value::Infinity(b) => write!(f, "{}inf", if *b { "-" } else { "" }),
            Value::NativeFunction(_) => write!(f, "native_function"),
            Value::Closure(c) => write!(f, "=>({})", c.function),
            Value::Function(ff) => write!(f, "{}", ff),
            Value::Table(t) => write!(f, "{}", t.borrow().to_string()),
            Value::UserData(u) => write!(f, "{}", u.to_string()),
            #[cfg(feature = "vectors")]
            Value::Vec3(v) => write!(f, "{}", v),
            #[cfg(feature = "vectors")]
            Value::Vec2(v) => write!(f, "{}", v),
        }
    }
}

impl core::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl<'v> Value<'v> {
    /** Condense value into a tiny enum for passing to errors*/
    pub fn to_error(&self) -> ValueTypes {
        match self {
            Value::Integer(_) => ValueTypes::Integer,
            Value::Number(_) => ValueTypes::Number,
            Value::Bool(_) => ValueTypes::Bool,
            Value::Nil => ValueTypes::Nil,
            Value::String(_) => ValueTypes::String,
            Value::Infinity(_) => ValueTypes::Infinity,
            Value::NativeFunction(_) => ValueTypes::NativeFunction,
            Value::Function { .. } => ValueTypes::Function,
            Value::Closure(_) => ValueTypes::Closure,
            Value::Table(_) => ValueTypes::Table,
            Value::UserData(_) => ValueTypes::UserData,
            #[cfg(feature = "vectors")]
            Value::Vec3(_) => ValueTypes::Vec3,
            #[cfg(feature = "vectors")]
            Value::Vec2(_) => ValueTypes::Vec2,
        }
    }

    pub fn force_to_int(&mut self, n: i64) {
        *self = Value::Integer(n);
    }
    pub fn force_to_float(&mut self, n: f64) {
        *self = Value::Number(n);
    }

    pub fn increment(&mut self, value: &Value) -> Result<(), SiltError> {
        binary_self_op!(self, +=,+, value, Add)
        // match match (&mut *self, value) {
        //     (Value::Number(left), Value::Number(right)) => {
        //         *left += right;
        //         None
        //     }
        //     (Value::Integer(left), Value::Number(right)) => {
        //         Some(Value::Number((*left as f64) + right))
        //         // self.force_to_float(left as f64 + right);
        //     }

        //     _ => unreachable!(),
        // } {
        //     Some(v) => *self = v,
        //     None => {}
        // }
        // Ok(())
    }

    pub fn clone(&self) -> Value<'v> {
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
            Value::Closure(c) => Value::Closure(Rc::clone(c)),
            // Value::Table(t) => Value::Table(Reference {
            //     value: Rc::clone(&t.value),
            //     id: t.id,
            // }),
            Value::Table(t) => Value::Table(Rc::clone(t)),
            Value::UserData(u) => Value::UserData(Rc::clone(u)),
            #[cfg(feature = "vectors")]
            Value::Vec3(v) => Value::Vec3(*v),
            #[cfg(feature = "vectors")]
            Value::Vec2(v) => Value::Vec2(*v),
        }
    }
}

impl PartialEq for Value<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Integer(i), Value::Integer(j)) => i == j,
            (Value::Number(i), Value::Number(j)) => i == j,
            (Value::Bool(i), Value::Bool(j)) => i == j,
            (Value::Nil, Value::Nil) => true,
            (Value::String(i), Value::String(j)) => i == j,
            (Value::Infinity(i), Value::Infinity(j)) => i == j,
            (Value::NativeFunction(i), Value::NativeFunction(j)) => {
                // i == j
                i as *const _ == j as *const _

                // i.function as *const for<'a> fn(&'a mut Lua, Vec<Value<'a>>) -> Value<'a>
                //     == j.function as *const for<'a> fn(&'a mut Lua, Vec<Value<'a>>) -> Value<'a>
            }
            (Value::Function(i), Value::Function(j)) => Rc::ptr_eq(i, j),
            (Value::Table(i), Value::Table(j)) => Rc::ptr_eq(&i, &j),
            _ => false,
        }
    }
}

impl Default for Value<'_> {
    fn default() -> Self {
        Value::Nil
    }
}

impl Clone for Value<'_> {
    fn clone(&self) -> Self {
        self.clone()
    }
}

impl Hash for Value<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl Eq for Value<'_> {}

pub trait ToLua<'lua> {
    /// Performs the conversion.
    fn to_lua(self, lua: &'lua Lua) -> Result<Value<'lua>, SiltError>;
}

/// Trait for types convertible from `Value`.
pub trait FromLua<'lua>: Sized {
    /// Performs the conversion.
    fn from_lua(lua_value: Value<'lua>, lua: &'lua Lua) -> Result<Self, SiltError>;
}

#[derive(Debug, Clone)]
pub struct MultiValue<'lua>(Vec<Value<'lua>>);

pub trait ToLuaMulti<'lua> {
    fn to_lua_multi(self, lua: &'lua Lua) -> Result<MultiValue<'lua>, SiltError>;
}

pub trait FromLuaMulti<'lua>: Sized {
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua Lua) -> Result<Self, SiltError>;
}
