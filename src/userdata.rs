use crate::{
    code::OpCode,
    prelude::{VM, LuaError},
    value::{FromLua, MultiValue, ToLua, Value},
};

pub trait UserData<'lua> {
    fn get_type(&self) -> String;
    fn to_string(&self) -> String;
    // fn add_methods<'lua, T: UserDataFields<'lua, Self>>(&self, _methods: &mut T);
    fn by_meta_method(
        &self,
        lua: &mut VM,
        method: MetaMethod,
        inputs: Value<'lua>,
    ) -> Result<Value<'lua>>;
    //dyn ToLuaMulti<'lua>;
    // fn get_meta_methods(&self) -> Option<MetaMethods> {
    //     None
    // }
}

// pub struct MetaMethods {
//     // pub add: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub sub: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub mul: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub div: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub pow: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub concat: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub eq: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub lt: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub le: Option<fn(&UserData, &UserData) -> UserData>,
//     // pub len: Option<fn(&UserData) -> UserData>,
//     // pub tostring: Option<fn(&UserData) -> UserData>,
//     // pub call: Option<fn(&UserData, Vec<UserData>) -> UserData>,
// }

// struct ExampleUserData {
//     value: i64,
// }

// impl UserData for ExampleUserData {
//     fn get_type(&self) -> String {
//         "ExampleUserData".to_string()
//     }
//     fn to_string(&self) -> String {
//         self.value.to_string()
//     }
// }
// use std::result::Result as StdResult;
/// A specialized `Result` type used by `mlua`'s API.
pub type Result<T> = std::result::Result<T, LuaError>;

pub(crate) type Callback<'lua, 'a> =
    Box<dyn Fn(&'lua VM, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'a>;
pub trait MaybeSend {}
impl<T> MaybeSend for T {}

/// Shamelessly lifted from the mlua crate
// pub trait UserDataFields<'lua, T: UserData> {
//     fn add_field_method_get<S, R, M>(&mut self, name: &S, method: M)
//     where
//         S: AsRef<[u8]> + ?Sized,
//         R: ToLua<'lua>,
//         M: 'static + MaybeSend + Fn(&'lua Lua, &T) -> Result<R>;
//     fn add_field_method_set<S, A, M>(&mut self, name: &S, method: M)
//     where
//         S: AsRef<[u8]> + ?Sized,
//         A: FromLua<'lua>,
//         M: 'static + MaybeSend + FnMut(&'lua Lua, &mut T, A) -> Result<()>;

//     fn add_field_function_get<S, R, F>(&mut self, name: &S, function: F)
//     where
//         S: AsRef<[u8]> + ?Sized,
//         R: ToLua<'lua>,
//         F: 'static + MaybeSend + Fn(&'lua Lua, dyn UserData) -> Result<R>;

//     fn add_field_function_set<S, A, F>(&mut self, name: &S, function: F)
//     where
//         S: AsRef<[u8]> + ?Sized,
//         A: FromLua<'lua>,
//         F: 'static + MaybeSend + FnMut(&'lua Lua, dyn UserData, A) -> Result<()>;

//     fn add_meta_field_with<S, R, F>(&mut self, meta: S, f: F)
//     where
//         S: Into<MetaMethod>,
//         F: 'static + MaybeSend + Fn(&'lua Lua) -> Result<R>,
//         R: ToLua<'lua>;

//     #[doc(hidden)]
//     fn add_field_getter(&mut self, _name: Vec<u8>, _callback: Callback<'lua, 'static>) {}

//     #[doc(hidden)]
//     fn add_field_setter(&mut self, _name: Vec<u8>, _callback: Callback<'lua, 'static>) {}
// }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetaMethod {
    /// +
    Add,
    /// -
    Sub,
    /// *
    Mul,
    /// /
    Div,
    /// %
    Mod,
    /// ^
    Pow,
    /// - (unary minus)
    Unm,
    /// //
    IDiv,
    /// &
    BAnd,
    /// |
    BOr,
    /// ~
    BXor,
    /// ~
    BNot,
    /// <<
    Shl,
    /// >>
    Shr,
    /// ..
    Concat,
    /// #
    Len,
    /// ==
    Eq,
    /// <
    Lt,
    /// <=
    Le,
    /// >
    Gt,
    /// >=
    Ge,
    /// []
    Index,
    /// []= x
    NewIndex,
    Call,     // (...)
    ToString, // tostring etc
    Pairs,    // pairs builtin fn
    IPairs,   // ipairs builtin fn
              // Iter,
              // Close
}

impl std::fmt::Display for MetaMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetaMethod::Add => write!(f, "+"),
            MetaMethod::Sub => write!(f, "-"),
            MetaMethod::Mul => write!(f, "*"),
            MetaMethod::Div => write!(f, "/"),
            MetaMethod::Mod => write!(f, "%"),
            MetaMethod::Pow => write!(f, "^"),
            MetaMethod::Unm => write!(f, "-"),
            MetaMethod::IDiv => write!(f, "//"),
            MetaMethod::BAnd => write!(f, "&"),
            MetaMethod::BOr => write!(f, "|"),
            MetaMethod::BXor => write!(f, "~"),
            MetaMethod::BNot => write!(f, "~"),
            MetaMethod::Shl => write!(f, "<<"),
            MetaMethod::Shr => write!(f, ">>"),
            MetaMethod::Concat => write!(f, ".."),
            MetaMethod::Len => write!(f, "#"),
            MetaMethod::Eq => write!(f, "=="),
            MetaMethod::Lt => write!(f, "<"),
            MetaMethod::Le => write!(f, "<="),
            MetaMethod::Gt => write!(f, ">"),
            MetaMethod::Ge => write!(f, ">="),
            MetaMethod::Index => write!(f, "[]"),
            MetaMethod::NewIndex => write!(f, "[]="),
            MetaMethod::Call => write!(f, "()"),
            MetaMethod::ToString => write!(f, "tostring"),
            MetaMethod::Pairs => write!(f, "pairs"),
            MetaMethod::IPairs => write!(f, "ipairs"),
        }
    }
}

impl Into<MetaMethod> for OpCode {
    fn into(self) -> MetaMethod {
        match self {
            OpCode::ADD => MetaMethod::Add,
            OpCode::SUB => MetaMethod::Sub,
            OpCode::MULTIPLY => MetaMethod::Mul,
            OpCode::DIVIDE => MetaMethod::Div,
            // OpCode::MODULO => MetaMethod::Mod,
            _ => unimplemented!(),
        }
    }
}

// pub trait UserDataMethods<'lua, T: UserData> {
//     fn add_method<S, A, R, M>(&mut self, name: &S, method: M)
//     where
//         S: AsRef<[u8]> + ?Sized,
//         A: FromLuaMulti<'lua>,
//         R: ToLuaMulti<'lua>,
//         M: 'static + MaybeSend + Fn(&'lua Lua, &T, A) -> Result<R>;

//     fn add_meta_method<S, A, R, M>(&mut self, meta: S, method: M)
//     where
//         S: Into<MetaMethod>,
//         A: FromLuaMulti<'lua>,
//         R: ToLuaMulti<'lua>,
//         M: 'static + MaybeSend + Fn(&'lua Lua, &T, A) -> Result<R>;
// }

pub trait ToLuaMulti<'lua> {
    fn to_lua_multi(self, lua: &'lua VM) -> Result<MultiValue<'lua>>;
}

pub trait FromLuaMulti<'lua>: Sized {
    fn from_lua_multi(values: MultiValue<'lua>, lua: &'lua VM) -> Result<Self>;
}
