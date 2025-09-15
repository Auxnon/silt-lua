use std::{
    hash::{Hash, Hasher},
    ops::Deref,
    rc::Rc,
};

use gc_arena::{lock::RefLock, Collect, Gc, Mutation};

#[cfg(feature = "vectors")]
use crate::vec::{Vec2, Vec3};
use crate::{
    error::{SiltError, ValueTypes},
    function::{Closure, FunctionObject, NativeFunctionRc, NativeFunctionRef, WrappedFn},
    lua::VM,
    prelude::UserData,
    table::Table,
    userdata::{MetaMethod, UserDataWrapper},
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

impl ExVal {
    pub fn into_value<'g>(
        &self,
        vm: &mut VM<'g>,
        mc: &Mutation<'g>,
    ) -> Result<Value<'g>, SiltError> {
        Ok(match self {
            ExVal::Nil => Value::Nil,
            ExVal::Bool(b) => Value::Bool(*b),
            ExVal::Number(n) => Value::Number(*n),
            ExVal::String(s) => Value::String(s.to_owned()),
            ExVal::Table(t) => vm.convert_table(mc, t)?,
            ExVal::Integer(i) => Value::Integer(*i),
            ExVal::Infinity(b) => Value::Infinity(*b),
            ExVal::UserData(_) => return Err(SiltError::VmValBadConvert(ValueTypes::UserData)),
            ExVal::Meta(_) => return Err(SiltError::VmValBadConvert(ValueTypes::Function)),
        })
    }
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
    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::NativeFunction(_) => "native_function".to_string(),
            Value::Function(f) => match &f.name {
                Some(s) => s.to_owned(),
                None => "generic_function".to_string(),
            },
            Value::Closure(_) => "closure".to_string(),
            Value::UserData(_) => "userdata".to_string(),
            Value::Infinity(b) => format!("{}inf", if *b { "" } else { "-" }),
            Value::Bool(b) => b.to_string(),
            Value::Nil => "nil".to_string(),
            Value::Table(_) => "table".to_string(),
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

    pub fn apply_userdata<T>(
        &mut self,
        mc: &Mutation<'v>,
        apply: impl FnMut(&T) -> Result<(), SiltError>,
    ) -> Result<(), SiltError>
    where
        T: UserData,
    {
        if let Value::UserData(udw) = self {
            return udw.borrow_mut(mc).downcast_mut(apply);
        }
        Err(SiltError::UDBadCast)
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

/// Trait for types convertible from `Value`.
pub trait FromLua<'lua>: Sized {
    fn from_lua(val: &Value<'lua>, lua: &VM<'lua>, mc: &Mutation<'lua>) -> Result<Self, SiltError>;
}

// impl<'a, 'b> FromLua<'a> for Value<'a> {
//     fn from_lua(val: &Value<'a>, _: &VM<'a>) -> Result<Value<'a>, SiltError> {
//         Ok(val.to_owned())
//     }
// }

// impl<'lua, 'b, A> FromLua<'lua> for A
// where
//     A: From<Value<'lua>>,
// {
//     fn from_lua(val: &Value<'lua>, _: &VM<'lua>) -> Result<Self, SiltError> {
//         Ok(val.into())
//     }
// }

// impl<'a> From<&Value<'a>> for Value<'a> {
//     fn from(value: &Value<'a>) -> Self {
//         value.clone()
//     }
// }

// impl<'a> Into<Value<'a>> for &Value<'a> {
//     fn into(self) -> Self {
//         &self
//     }
// }

// impl<'a, A> Into<A> for &Value<'a> where A: Sized{
//     fn into(self) -> A {
//         (&*self).into()
//     }
// }

// ========== convert i64 ==========
impl From<i64> for Value<'_> {
    fn from(value: i64) -> Self {
        Value::Integer(value)
    }
}

impl From<Value<'_>> for i64 {
    fn from(value: Value<'_>) -> i64 {
        match value {
            // TODO is this lossless conversion best?
            Value::Number(f) => f.max(i64::MIN as f64).min(i64::MAX as f64).round() as i64,
            Value::Integer(i) => i,
            // TODO Value::String()
            _ => 0,
        }
    }
}

impl From<&Value<'_>> for i64 {
    fn from(value: &Value<'_>) -> i64 {
        match value {
            // TODO is this lossless conversion best?
            Value::Number(f) => f.max(i64::MIN as f64).min(i64::MAX as f64).round() as i64,
            Value::Integer(i) => *i,
            // TODO Value::String()
            _ => 0,
        }
    }
}

// ========== convert u32 ==========
impl From<u32> for Value<'_> {
    fn from(value: u32) -> Self {
        Value::Integer(value.into())
    }
}

impl From<Value<'_>> for u32 {
    fn from(value: Value<'_>) -> u32 {
        match value {
            // TODO is this lossless conversion best?
            Value::Number(f) => f.max(u32::MIN as f64).min(u32::MAX as f64).round() as u32,
            Value::Integer(i) => i.max(u32::MAX as i64).min(0) as u32,
            // TODO Value::String()
            _ => 0,
        }
    }
}

impl From<&Value<'_>> for u32 {
    fn from(value: &Value<'_>) -> u32 {
        match value {
            // TODO is this lossless conversion best?
            Value::Number(f) => f.max(u32::MIN as f64).min(u32::MAX as f64).round() as u32,
            Value::Integer(i) => (*i).max(u32::MAX as i64).min(0) as u32,
            // TODO Value::String()
            _ => 0,
        }
    }
}

impl<'a> FromLua<'a> for u32 {
    fn from_lua(val: &Value<'a>, _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

// ========== convert i32 ==========
impl From<i32> for Value<'_> {
    fn from(value: i32) -> Self {
        Value::Integer(value.into())
    }
}

impl From<Value<'_>> for i32 {
    fn from(value: Value<'_>) -> i32 {
        match value {
            Value::Number(f) => f.max(i32::MIN as f64).min(i32::MAX as f64).round() as i32,
            Value::Integer(i) => i.max(i32::MIN as i64).min(i32::MAX as i64) as i32,
            _ => 0,
        }
    }
}

impl From<&Value<'_>> for i32 {
    fn from(value: &Value<'_>) -> i32 {
        match value {
            Value::Number(f) => f.max(i32::MIN as f64).min(i32::MAX as f64).round() as i32,
            Value::Integer(i) => (*i).max(i32::MIN as i64).min(i32::MAX as i64) as i32,
            _ => 0,
        }
    }
}

impl<'a> FromLua<'a> for i32 {
    fn from_lua(val: &Value<'a>, _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

// ========== convert f64 ==========
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
            // TODO Value::String()
            _ => 0.,
        }
    }
}

impl From<&Value<'_>> for f64 {
    fn from(value: &Value<'_>) -> f64 {
        match value {
            Value::Number(f) => *f,
            Value::Integer(i) => *i as f64,
            // TODO Value::String()
            _ => 0.,
        }
    }
}

impl FromLua<'_> for f64 {
    fn from_lua(val: &Value<'_>, _: &VM<'_>, _: &Mutation<'_>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

// ========== convert bool ==========
impl From<bool> for Value<'_> {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<Value<'_>> for bool {
    fn from(value: Value<'_>) -> Self {
        match value {
            // TODO can we eval 0 as false or will that break too much?
            // Value::Number(f) => f!=0.,
            // Value::Integer(i) => 0!=0,
            Value::Bool(b) => b,
            Value::Nil => false,
            _ => true,
        }
    }
}

impl From<&Value<'_>> for bool {
    fn from(value: &Value<'_>) -> Self {
        match value {
            Value::Bool(b) => *b,
            Value::Nil => false,
            _ => true,
        }
    }
}

impl FromLua<'_> for bool {
    fn from_lua(val: &Value<'_>, _: &VM<'_>, _: &Mutation<'_>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

// impl<'lua, 'b> FromLua<'lua> for i64 {
//     fn from_lua(val: &Value<'lua>, _: &VM<'lua>) -> Result<Self, SiltError> {
//         Ok(val.into())
//     }
// }

// ========== convert &str ==========
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

// ========== convert String ==========
impl From<String> for Value<'_> {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}
impl From<Value<'_>> for String {
    fn from(val: Value) -> Self {
        val.to_string()
    }
}
impl From<&Value<'_>> for String {
    fn from(val: &Value) -> Self {
        val.to_string()
    }
}

impl FromLua<'_> for String {
    fn from_lua(val: &Value<'_>, _: &VM<'_>, _: &Mutation<'_>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

// ========== convert () ==========
impl From<()> for Value<'_> {
    fn from(_: ()) -> Self {
        Value::Nil
    }
}

// ========== convert u8 ==========
impl From<&Value<'_>> for u8 {
    fn from(value: &Value<'_>) -> u8 {
        match value {
            // TODO is this lossless conversion best?
            Value::Number(f) => f.clamp(0.0, 255.0) as u8,
            Value::Integer(i) => (*i).clamp(0, 255) as u8,
            // TODO Value::String()
            _ => 0,
        }
    }
}

impl<'a> From<&Value<'a>> for Option<Value<'a>> {
    fn from(value: &Value<'a>) -> Self {
        match value {
            Value::Nil => None,
            a => Some(a.to_owned()),
        }
    }
}
impl<'a> FromLua<'a> for Option<Value<'a>> {
    fn from_lua(val: &Value<'a>, _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(val.into())
    }
}

impl<'a> FromLua<'a> for Value<'a> {
    fn from_lua(val: &Value<'a>, _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(val.clone())
    }
}

// impl From<Value<'_>> for &mut UserDataWrapper{
//     fn from(value: Value<'_>) -> Self {
//         if let Value::UserData(ud)=value {
//             ud.borrow_mut(mc)
//
//         }
//     }
// }

impl Eq for Value<'_> {}

pub trait ToLua<'a> {
    fn to_lua(self, lua: &VM<'a>, mc: &Mutation<'a>) -> Result<Value<'a>, SiltError>;
}

impl<'a, A> ToLua<'a> for A
where
    A: Into<Value<'a>>,
{
    fn to_lua(self, _: &VM<'a>, _: &Mutation<'a>) -> Result<Value<'a>, SiltError> {
        Ok(self.into())
    }
}
// impl<'a> ToLua<'a> for Value<'a> {
//     fn to_lua(self, _: &VM<'a>, _: &Mutation<'a>) -> Result<Value<'a>, SiltError> {
//         Ok(self)
//     }
// }
// impl<'a> ToLua<'a> for Result<Value<'a>, SiltError> {
//     fn to_lua(self, _: &VM<'a>, _: &Mutation<'a>) -> Result<Value<'a>, SiltError> {
//         self
//     }
// }
impl<'a, A> ToLua<'a> for Result<A, SiltError>
where
    A: Into<Value<'a>>,
{
    fn to_lua(self, _: &VM<'a>, _: &Mutation<'a>) -> Result<Value<'a>, SiltError> {
        match self {
            Err(e) => Err(e),
            Ok(v) => Ok(v.into()),
        }
    }
}
// #[derive(Debug, Clone)]
// pub struct MultiValue<'lua>(Vec<Value<'lua>>);
type ValueResult<'a> = Result<Value<'a>, SiltError>;
type Values<'a> = Vec<Value<'a>>;
type ValuesResult<'a> = Result<Values<'a>, SiltError>;

pub trait ToLuaMulti<'a> {
    fn to_lua_multi(self, lua: &VM<'a>, mc: &Mutation<'a>) -> ValuesResult<'a>;
}

impl<'a> ToLuaMulti<'a> for Vec<()> {
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'a>) -> ValuesResult<'a> {
        Ok(vec![])
    }
}

impl<'a, A, I> ToLuaMulti<'a> for I
where
    I: IntoIterator<Item = A>,
    A: Into<Value<'a>>,
    I: NotSingle,
{
    fn to_lua_multi(self, _: &VM<'a>, _: &Mutation) -> ValuesResult<'a> {
        let mut bucket = vec![];
        for v in self.into_iter() {
            bucket.push(v.into())
        }
        Ok(bucket)
    }
}

trait NotSingle {}
// impl<T> NotSingle for Vec<T>{}
// impl<T, const N: usize> NotSingle for [ T; N ]{}
// impl<T> NotSingle for &[T] {}
// impl NotSingle for std::ops::Range<i32>{}
// impl NotSingle for std::ops::Range<usize>{}
// impl NotSingle for std::ops::Range<i64>{}
// impl NotSingle for std::ops::RangeInclusive<i32>{}
// impl NotSingle for std::ops::RangeInclusive<usize>{}
// impl NotSingle for std::ops::RangeInclusive<i64>{}

// impl<'a, A> ToLuaMulti<'a> for A
// where
//     A: Into<Value<'a>>,
// {
//     fn to_lua_multi(self, _: &VM<'a>, _: &Mutation<'a>) -> ValuesResult<'a> {
//         Ok(vec![self.into()])
//     }
// }
//

impl<'a> ToLuaMulti<'a> for () {
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'a>) -> ValuesResult<'a> {
        Ok(vec![])
    }
}

// impl<'a> ToLuaMulti<'a> for Vec<()> {
//     fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
//         Ok(vec![])
//     }
// }

// impl<'a, A> ToLuaMulti<'a> for A
// where
//     A: Into<Value<'a>> ,
// {
//     fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
//         Ok(vec![self.into()])
//     }
// }

impl<'a, A> ToLua<'a> for Vec<A>
where
    A: Into<Value<'a>>,
{
    fn to_lua(self, vm: &VM<'a>, mc: &Mutation<'a>) -> ValueResult<'a> {
        if self.len() == 0 {
            return Ok(vm.new_table(mc));
        }
        let mut t = vm.raw_table();
        t.concat_array(self);
        Ok(vm.wrap_table(mc, t))
    }
}

impl<'a, A, B> ToLuaMulti<'a> for (A, B)
where
    A: Into<Value<'a>>,
    B: Into<Value<'a>>,
{
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
        Ok(vec![self.0.into(), self.1.into()])
    }
}

impl<'a, A, B, C> ToLuaMulti<'a> for (A, B, C)
where
    A: Into<Value<'a>>,
    B: Into<Value<'a>>,
    C: Into<Value<'a>>,
{
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
        Ok(vec![self.0.into(), self.1.into(), self.2.into()])
    }
}

impl<'a, A, B, C, D> ToLuaMulti<'a> for (A, B, C, D)
where
    A: Into<Value<'a>>,
    B: Into<Value<'a>>,
    C: Into<Value<'a>>,
    D: Into<Value<'a>>,
{
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
        Ok(vec![
            self.0.into(),
            self.1.into(),
            self.2.into(),
            self.3.into(),
        ])
    }
}

impl<'a, A, B, C, D, E> ToLuaMulti<'a> for (A, B, C, D, E)
where
    A: Into<Value<'a>>,
    B: Into<Value<'a>>,
    C: Into<Value<'a>>,
    D: Into<Value<'a>>,
    E: Into<Value<'a>>,
{
    fn to_lua_multi<'e>(self, _: &VM<'a>, _: &Mutation<'e>) -> ValuesResult<'a> {
        Ok(vec![
            self.0.into(),
            self.1.into(),
            self.2.into(),
            self.3.into(),
            self.4.into(),
        ])
    }
}

pub trait FromLuaMulti<'a>: Sized {
    fn from_lua_multi<'b>(
        args: &'b [Value<'a>],
        lua: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError>;
}

// TODO how to return... the same args again?
// impl<'a,'c> FromLuaMulti<'a> for &'c [Value<'a>]{
//     fn from_lua_multi<'b>(
//         args: &'b [Value<'a>],
//         lua: &VM<'a>,
//         mc: &Mutation<'a>,
//     ) -> Result<Self, SiltError> {
//         Ok(args)
//     }
// }

impl<'a> FromLuaMulti<'a> for Vec<Value<'a>> {
    fn from_lua_multi(args: &[Value<'a>], _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(args.to_vec())
    }
}

impl<'a> FromLuaMulti<'a> for () {
    fn from_lua_multi(_: &[Value<'a>], _: &VM<'a>, _: &Mutation<'a>) -> Result<Self, SiltError> {
        Ok(())
    }
}

impl<'a, T1> FromLuaMulti<'a> for (T1,)
where
    // T1: From<Value<'a>>,
    T1: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        // if self.len() != 1 {
        //     return Err(SiltError::VmNativeParameterMismatch);
        // }
        // T1::from
        Ok((T1::from_lua(&args[0], vm, mc)?,))
    }
}

impl<'a, A, B> FromLuaMulti<'a> for (A, B)
where
    A: FromLua<'a>,
    B: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        Ok((
            A::from_lua(&args[0], vm, mc)?,
            B::from_lua(&args[1], vm, mc)?,
        ))
    }
}

impl<'a, A, B, C> FromLuaMulti<'a> for (A, B, C)
where
    A: FromLua<'a>,
    B: FromLua<'a>,
    C: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        Ok((
            A::from_lua(&args[0], vm, mc)?,
            B::from_lua(&args[1], vm, mc)?,
            C::from_lua(&args[2], vm, mc)?,
        ))
    }
}

impl<'a, A, B, C, D> FromLuaMulti<'a> for (A, B, C, D)
where
    A: FromLua<'a>,
    B: FromLua<'a>,
    C: FromLua<'a>,
    D: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        Ok((
            A::from_lua(&args[0], vm, mc)?,
            B::from_lua(&args[1], vm, mc)?,
            C::from_lua(&args[2], vm, mc)?,
            D::from_lua(&args[3], vm, mc)?,
        ))
    }
}

impl<'a, A, B, C, D, E> FromLuaMulti<'a> for (A, B, C, D, E)
where
    A: FromLua<'a>,
    B: FromLua<'a>,
    C: FromLua<'a>,
    D: FromLua<'a>,
    E: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        Ok((
            A::from_lua(&args[0], vm, mc)?,
            B::from_lua(&args[1], vm, mc)?,
            C::from_lua(&args[2], vm, mc)?,
            D::from_lua(&args[3], vm, mc)?,
            E::from_lua(&args[4], vm, mc)?,
        ))
    }
}

impl<'a, A, B, C, D, E, F> FromLuaMulti<'a> for (A, B, C, D, E, F)
where
    A: FromLua<'a>,
    B: FromLua<'a>,
    C: FromLua<'a>,
    D: FromLua<'a>,
    E: FromLua<'a>,
    F: FromLua<'a>,
{
    fn from_lua_multi(
        args: &[Value<'a>],
        vm: &VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Self, SiltError> {
        Ok((
            A::from_lua(&args[0], vm, mc)?,
            B::from_lua(&args[1], vm, mc)?,
            C::from_lua(&args[2], vm, mc)?,
            D::from_lua(&args[3], vm, mc)?,
            E::from_lua(&args[4], vm, mc)?,
            F::from_lua(&args[5], vm, mc)?,
        ))
    }
}

// pub trait FromLuaMulti<'lua>: Sized {
//     fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua VM) -> Result<Self, SiltError>;
// }

// pub trait FromValue: Sized {
//     fn from_value(value: Value) -> Result<Self, SiltError>;
// }
#[derive(Debug)]
pub struct Variadic<'a, T> {
    values: Vec<Value<'a>>,
    _phantom: std::marker::PhantomData<T>,
}
