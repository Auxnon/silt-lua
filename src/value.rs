use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    rc::Rc,
};

use gc_arena::{lock::RefLock, Collect, Gc, Mutation};

#[cfg(feature = "vectors")]
use crate::vec::{Vec2, Vec3};
use crate::{
    error::{SiltError, ValueTypes}, function::{Closure, FunctionObject, NativeFunctionRc, NativeFunctionRef, WrappedFn}, lua::VM, prelude::UserData, table::Table, userdata::{MetaMethod, UserDataWrapper}
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

// trait Testo {
//     fn yes(&self);
// }

/** Lua value enum representing different data types within a VM */
#[derive(Collect, Default)]
#[collect(no_drop)]
#[repr(u8)]
pub enum Value<'gc> {
    #[default]
    Nil,
    Integer(i64),
    Number(f64),
    Bool(bool),
    /** true for negative */
    Infinity(bool),
    // Bool(bool),
    // TODO in most cases strings are just copied around on the heap, this is expensive but saves us from using GC here
    // TODO 2 consider other encoding options for efficiency. Is having a seperate ASCII string type beneficial to only english speakers? how would other speaking languages handle ascii strings without needed glyphs?
    String(String),
    // List(Vec<Value>),
    // Map(HashMap<String, Value>),
    Table(Gc<'gc, RefLock<Table<'gc>>>),
    // Array // TODO lua 5 has an actual array type chosen contextually, how much faster can we make a table by using it?
    // Boxed()
    Function(Gc<'gc, FunctionObject<'gc>>), // closure: Environment,
    Closure(Gc<'gc, Closure<'gc>>),
    // Func(fn(Vec<Value>) -> Value)
    // NativeFunction(Gc<'gc, WrappedFn<'gc>>),
    NativeFunction(Gc<'gc, WrappedFn<'gc>>),
    UserData(Gc<'gc, RefLock<UserDataWrapper>>),
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
    String(String),
    Table(crate::table::ExTable),
    Meta(String),
    UserData(String),
    #[cfg(feature = "vectors")]
    Vec3(Vec3),
    #[cfg(feature = "vectors")]
    Vec2(Vec2),
}

pub enum MultiVal<'a, 'b> {
    Vecced(&'a [Value<'b>]),
    Single(&'a Value<'b>),
}

impl<'a, 'b> Into<&'a Value<'b>> for MultiVal<'a, 'b> {
    fn into(self) -> &'a Value<'b> {
        match self {
            MultiVal::Vecced(a) => {
                if !a.is_empty() {
                    &a[0]
                } else {
                    &Value::Nil
                }
            }
            MultiVal::Single(v) => v,
        }
    }
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
            Value::NativeFunction(_) => ExVal::Meta("native_function".to_string()),
            Value::UserData(u) => ExVal::UserData(format!("{} userdata", u.borrow().type_name())),
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
            ExVal::UserData(u) => write!(f, "{}", u.to_string()),
            #[cfg(feature = "vectors")]
            ExVal::Vec3(v) => write!(f, "{}", v),
            #[cfg(feature = "vectors")]
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
            Value::Table(t) => write!(f, "table[;{}]", t.borrow().len()),
            Value::UserData(_) => write!(f, "userdata"), // TODO
            #[cfg(feature = "vectors")]
            Value::Vec3(v) => write!(f, "{}", v),
            #[cfg(feature = "vectors")]
            Value::Vec2(v) => write!(f, "{}", v),
            // Value::UserData(u) => write!(f, "{}", v),
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
    /** normal to_string takes some liberties for convenient display purposes. This more raw
     * approach is used for UserData hashmap lookup*/
    pub fn pure_string(&self) -> String {
        match self {
            // Value::Integer(i) => i.to_string(),
            // Value::Number(n) => write!(f, "{}", n),
            // Value::Bool(b) => write!(f, "{}", b),
            // Value::Nil => write!(f, "nil"),
            // Value::String(s) => write!(f, "\"{}\"", s),
            // Value::Infinity(b) => write!(f, "{}inf", if *b { "-" } else { "" }),
            // Value::NativeFunction(_) => write!(f, "native_function"),
            // Value::Closure(c) => write!(f, "=>({})", c.function),
            // Value::Function(ff) => write!(f, "{}", ff),
            // Value::Table(_t) => write!(f, "{}", 't'),
            // Value::UserData(u) => write!(f, "userdata")
            Value::String(s) => s.to_string(),
            _ => "NA".to_string(),
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
            Value::Function(r) => Value::Function(Gc::clone(r)),
            Value::Closure(c) => Value::Closure(Gc::clone(c)),
            // Value::Table(t) => Value::Table(Reference {
            //     value: Rc::clone(&t.value),
            //     id: t.id,
            // }),
            Value::Table(t) => Value::Table(Gc::clone(t)),
            Value::UserData(u) => Value::UserData(Gc::clone(u)),
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
            (Value::Function(i), Value::Function(j)) => Gc::ptr_eq(*i, *j), // Rc::ptr_eq(i, j),

            (Value::Table(i), Value::Table(j)) => Gc::ptr_eq(*i, *j),
            _ => false,
        }
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
// impl Into<i64> for Value<'_> {
//     fn into(self) -> i64 {
//         match self {
//             Value::Integer(i) => i,
//             _ => 0,
//         }
//     }
// }

impl Deref for Value<'_> {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        match self {
            Value::Integer(i) => &i,
            _ => &0,
        }
    }
}

// impl<'a> Into<f64> for Value<'a> {
//     fn into(self) -> f64 {
//         match self {
//             Value::Number(f) => f,
//             Value::Integer(i) => i as f64,
//             _ => 0.,
//         }
//     }
// }

impl From<i64> for Value<'_> {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}
impl From<&str> for Value<'_> {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<&String> for Value<'_> {
    fn from(value: &String) -> Self {
        Value::String(value.to_string())
    }
}
impl From<String> for Value<'_> {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<()> for Value<'_> {
    fn from(_: ()) -> Self {
        Value::Nil
    }
}
impl From<f64> for Value<'_> {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl From<Value<'_>> for f64 {
    fn from(value: Value<'_>) -> f64 {
        match value {
            Value::Number(f) => f,
            Value::Integer(i) => i as f64,
            _ => 0.,
        }
    }
}

impl From<&Value<'_>> for f64 {
    fn from(value: &Value<'_>) -> f64 {
        match value {
            Value::Number(f) => *f,
            Value::Integer(i) => *i as f64,
            _ => 0.,
        }
    }
}

impl Eq for Value<'_> {}

pub trait ToLua<'lua>: Sized + 'static {
    fn to_lua(self, lua: &'lua VM, mc: &Mutation) -> Result<Value<'lua>, SiltError>;
}

/// Trait for types convertible from `Value`.
pub trait FromLua<'lua>: Sized {
    fn from_lua(lua_value: Value<'lua>, lua: &'lua VM) -> Result<Self, SiltError>;
}

#[derive(Debug, Clone)]
pub struct MultiValue<'lua>(Vec<Value<'lua>>);

pub trait ToLuaMulti<'lua> {
    fn to_lua_multi(self, lua: &'lua VM) -> Result<MultiValue<'lua>, SiltError>;
}

pub trait FromLuaMulti<'lua>: Sized {
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua VM) -> Result<Self, SiltError>;
}


// pub trait FromValue: Sized {
//     fn from_value(value: Value) -> Result<Self, SiltError>;
// }
#[derive(Debug)]
pub struct Variadic<'a,T> {
    values: Vec<Value<'a>>,
    _phantom: std::marker::PhantomData<T>,
}
