use std::collections::HashMap;

use crate::{
    code::OpCode,
    prelude::{LuaError, VM},
    value::{MultiValue,  Value},
};

pub trait UserData {
    /// unique name for userdata to distinguish it from others in lookup
    fn get_type(&self) -> &str;
    /// lua's stringify used if metamethod not set, defaults to get_type output
    fn to_string(&self) -> &str {
        self.get_type()
    }
    // fn add_methods<'lua, T: UserDataFields<'lua, Self>>(&self, _methods: &mut T);
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(_methods: &mut M)
    where
        Self: Sized,
    {
    }
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(_fields: &mut F)
    where
        Self: Sized{}

    // fn by_meta_method(
    //     &self,
    //     lua: &mut VM,
    //     method: MetaMethod,
    //     inputs: Value<'lua>,
    // ) -> Result<Value<'lua>>;
    //dyn ToLuaMulti<'lua>;
    // fn get_meta_methods(&self) -> Option<MetaMethods> {
    //     None
    // }
}

// trait IV<'v> = Into<Value<'v>>;

pub struct UserDataMethodsPayload<'v> {
    methods: HashMap<Value<'v>, Value<'v>>,
}

pub trait UserDataFields<'v, S> {
    fn add_field_method_get<F, R>(&mut self, _name: &str, _closure: F)
    where
        R: Into<Value<'v>>,
        F: Fn(VM<'v>, S) -> Result<R>,
    {
    }

    fn add_field_method_set<T, F, R>(&mut self, _name: &str, _closure: F)
    where
        T: Into<Value<'v>>,
        R: Into<Value<'v>>,
        F: Fn(VM<'v>, S, T) -> Result<R>,
    {
    }
}

// type IV<'v> = Into<Value<'v>>;
pub trait UserDataMethods<'v, S> {
    fn add_meta_method<T, M, R>(&mut self, name: &str, closure: M)
    where
        T: Into<Value<'v>>,
        R: Into<Value<'v>>,
        M: Fn(VM<'v>, S, T) -> Result<R>;

    fn add_method_mut<T, M, R>(&mut self, name: &str, closure: &dyn Fn(VM<'v>, S, T) -> Result<R>)
    where
        T: Into<Value<'v>>,
        R: Into<Value<'v>>,
        M: Fn(VM<'v>, S, T) -> Result<T>;
}

pub struct UserDataFieldsPayload<'v> {
    getters: HashMap<Value<'v>, Value<'v>>,
    setters: HashMap<Value<'v>, Value<'v>>,
}


////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct Ent {
    x: f64,
    y: f64,
    z: f64,
}

impl Ent {
    pub fn get_id(&self) -> i64 {
        0
    }
}

impl UserData for Ent {
    fn get_type(&self) -> &str {
        "ent"
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, _: ()| {
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_meta_method("__concat", |_, this, _: ()| {
            Ok(format!("[entity {}]", this.get_id()))
        });
        // methods.add_method_mut("pos", |_, this, p: (f64, f64, f64)| {
        //     this.x = p.0;
        //     this.y = p.1;
        //     this.z = p.2;
        //
        //     Ok(())
        // });
    }

    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, this| Ok(this.x));
        fields.add_field_method_set("x", |_, mut this, x: f64| {
            this.x = x;
            Ok(Value::Nil)
        });

        fields.add_field_method_get("y", |_, this| Ok(this.y));
        fields.add_field_method_set("y", |_, mut this, y: f64| {
            this.y = y;
            Ok(Value::Nil)
        });

        fields.add_field_method_get("z", |_, this| Ok(this.z));
        fields.add_field_method_set("z", |_, mut this, z: f64| {
            this.z = z;
            Ok(Value::Nil)
        });
        //
        //     // fields.add_field_method_get("rx", |_, this| Ok(this.rot_x));
        //     // fields.add_field_method_get("ry", |_, this| Ok(this.rot_y));
        //     // fields.add_field_method_get("rz", |_, this| Ok(this.rot_z));
    }
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

// pub(crate) type Callback<'lua, 'a> =
//     Box<dyn Fn(&'lua VM, MultiValue<'lua>) -> Result<MultiValue<'lua>> + 'a>;
// pub trait MaybeSend {}
// impl<T> MaybeSend for T {}

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

impl MetaMethod {
    pub fn to_table_key(&self) -> &'static str {
        match self {
            MetaMethod::Add => "__add",
            MetaMethod::Sub => "__sub",
            MetaMethod::Mul => "__mul",
            MetaMethod::Div => "__div",
            MetaMethod::Mod => "__mod",
            MetaMethod::Pow => "__pow",
            MetaMethod::Unm => "__unm",
            MetaMethod::IDiv => "__idiv",
            MetaMethod::BAnd => "__band",
            MetaMethod::BOr => "__bor",
            MetaMethod::BXor => "__bxor",
            MetaMethod::BNot => "__bnot",
            MetaMethod::Shl => "__shl",
            MetaMethod::Shr => "__shr",
            MetaMethod::Concat => "__concat",
            MetaMethod::Len => "__len",
            MetaMethod::Eq => "__eq",
            MetaMethod::Lt => "__lt",
            MetaMethod::Le => "__le",
            MetaMethod::Gt => "__gt",
            MetaMethod::Ge => "__ge",
            MetaMethod::Index => "__index",
            MetaMethod::NewIndex => "__newindex",
            MetaMethod::Call => "__call",
            MetaMethod::ToString => "__tostring",
            MetaMethod::Pairs => "__pairs",
            MetaMethod::IPairs => "__ipairs",
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
