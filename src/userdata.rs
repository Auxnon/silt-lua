use crate::{
    prelude::{Lua, LuaError},
    value::{FromLua, MultiValue, ToLua},
};

pub trait UserData {
    fn get_type(&self) -> String;
    fn to_string(&self) -> String;
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
    Box<dyn Fn(&'lua Lua, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'a>;
pub trait MaybeSend {}
impl<T> MaybeSend for T {}
/// Shamelessly lifted from the mlua crate
pub trait UserDataFields<'lua, T: UserData> {
    fn add_field_method_get<S, R, M>(&mut self, name: &S, method: M)
    where
        S: AsRef<[u8]> + ?Sized,
        R: ToLua<'lua>,
        M: 'static + MaybeSend + Fn(&'lua Lua, &T) -> Result<R>;
    fn add_field_method_set<S, A, M>(&mut self, name: &S, method: M)
    where
        S: AsRef<[u8]> + ?Sized,
        A: FromLua<'lua>,
        M: 'static + MaybeSend + FnMut(&'lua Lua, &mut T, A) -> Result<()>;

    fn add_field_function_get<S, R, F>(&mut self, name: &S, function: F)
    where
        S: AsRef<[u8]> + ?Sized,
        R: ToLua<'lua>,
        F: 'static + MaybeSend + Fn(&'lua Lua, dyn UserData) -> Result<R>;

    fn add_field_function_set<S, A, F>(&mut self, name: &S, function: F)
    where
        S: AsRef<[u8]> + ?Sized,
        A: FromLua<'lua>,
        F: 'static + MaybeSend + FnMut(&'lua Lua, dyn UserData, A) -> Result<()>;

    fn add_meta_field_with<S, R, F>(&mut self, meta: S, f: F)
    where
        S: Into<MetaMethod>,
        F: 'static + MaybeSend + Fn(&'lua Lua) -> Result<R>,
        R: ToLua<'lua>;

    #[doc(hidden)]
    fn add_field_getter(&mut self, _name: Vec<u8>, _callback: Callback<'lua, 'static>) {}

    #[doc(hidden)]
    fn add_field_setter(&mut self, _name: Vec<u8>, _callback: Callback<'lua, 'static>) {}
}

#[derive(Debug, Clone)]
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
