use std::{any::TypeId, collections::HashMap, marker::PhantomData};

use crate::{
    code::OpCode,
    prelude::{LuaError, VM},
    value::{MultiValue, Value},
};

type LuaResult<'v> = Result<Value<'v>>;

pub trait UserData {
    /// unique name for userdata to distinguish it from others in lookup
    fn get_type() -> &'static str
    where
        Self: Sized;
    /// lua's stringify used if metamethod not set, defaults to get_type output
    fn to_stringy(&self) -> String;
    //     let u=UserData::get_type();
    //         'test'.to_string()
    // }
    // fn populate() {}

    // fn add_methods<'lua, T: UserDataFields<'lua, Self>>(&self, _methods: &mut T);
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(_methods: &mut M)
    where
        Self: Sized,
    {
    }
    fn add_fields<'v, F: UserDataFields<'v, Self>>(_fields: &mut F)
    where
        Self: Sized,
    {
    }

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
// type ValueResult = Result<Value<'v>>
// trait CC =  Fn(VM<'v>, S, impl Into<Value<'v>>) -> LuaResult<'v>
// type Closer = impl Fn(VM<'v>, S, impl Into<Value<'v>>) -> LuaResult<'v>
// type C =  impl Fn(VM<'v>, S, Value<'v>) -> Result<Value<'v>>

struct FromVal<'a>(Value<'a>);

impl<'v> From<()> for FromVal<'v> {
    fn from(_: ()) -> FromVal<'v> {
        FromVal(Value::Nil)
    }
}

impl From<i64> for FromVal<'_> {
    fn from(value: i64) -> Self {
        FromVal(Value::Integer(value))
    }
}

impl Into<i64> for FromVal<'_> {
    fn into(self) -> i64 {
        match self.0 {
            Value::Integer(i) => i,
            _ => 0,
        }
    }
}
pub trait UserDataMethods<'v, T: UserData> {
    fn add_meta_method<M>(&mut self, name: &str, closure: M)
    where
        M: Fn(VM<'v>, &T, Value<'v>) -> LuaResult<'v>;

    fn add_method_mut<M>(&mut self, name: &str, closure: M)
    where
        M: Fn(VM<'v>, &mut T, Value<'v>) -> LuaResult<'v>;
}
pub trait UserDataFields<'v, T: UserData> {
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(VM<'v>, &T) -> LuaResult<'v>;

    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(VM<'v>, &mut T, Value<'v>) -> LuaResult<'v>;
}
// macro_rules! Closur {
//     () => {impl Fn(VM<'v>, S, Value<'v>) -> Result<Value<'v>>};
// }

struct UserDataMethodsStruct {
    methods: HashMap<String, for<'v> fn(VM<'v>, &T, Value<'v>) -> LuaResult<'v>>,
}

// type IV<'v> = Into<Value<'v>>;
impl UserDataMethodsStruct
// where
//     M: Fn(VM<'v>, dyn UserData, Value<'v>) -> Result<Box<Value<'v>>>,
{
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }
}

struct UserDataFieldsStruct {
    pub setters:
        HashMap<String, for<'v> fn(&VM<'v>, &mut dyn UserData, Value<'v>) -> LuaResult<'v>>,
    pub getters: HashMap<String, for<'v> fn(&VM<'v>, &dyn UserData) -> LuaResult<'v>>,
}

impl<'v> UserDataFieldsStruct {
    pub fn new() -> Self {
        Self {
            getters: HashMap::new(),
            setters: HashMap::new(),
        }
    }
}

type FromVal2<'a> = dyn Into<Value<'a>>;

impl<'v, T: UserData> UserDataMethods<'v, T> for UserDataMethodsStruct {
    fn add_meta_method<M>(&mut self, _name: &str, _closure: M)
    where
        M: Fn(VM<'v>, &T, Value<'v>) -> Result<Value<'v>>,
    {
    }

    fn add_method_mut<M>(&mut self, _name: &str, _closure: M)
    where
        M: Fn(VM<'v>, &mut T, Value<'v>) -> Result<Value<'v>>,
    {
    }
}

impl<'v, T: UserData> UserDataFields<'v, T> for UserDataFieldsStruct {
    fn add_field_method_get<F>(&mut self, _name: &str, _closure: F)
    where
        F: Fn(VM<'v>, &T) -> Result<Value<'v>>,
    {
    }
    fn add_field_method_set<F>(&mut self, _name: &str, _closure: F)
    where
        F: Fn(VM<'v>, &mut T, Value<'v>) -> Result<Value<'v>>,
    {
    }
}

// impl<'v, S> UserDataMethods<'v, M> for UserDataMethodMap<'v> {
//     fn add_method_mut<T, M, R>(&mut self, name: &str, closure: M)
//     where
//         T: Into<Value<'v>>,
//         R: Into<Value<'v>>,
//         M: Fn(VM<'v>, S, T) -> Result<R>,
//     {
//         // self.methods.insert(name, closure);
//     }
//
//     fn add_meta_method<T, M, R>(&mut self, name: &str, closure: M)
//     where
//         T: Into<Value<'v>>,
//         R: Into<Value<'v>>,
//         M: Fn(VM<'v>, S, T) -> Result<R>,
//     {
//     }
// }
// trait LuaClosure<'v,S>=  Fn(VM<'v>, S) -> Result<Into<Value<'v>>>;
// pub struct UserDataFieldMap<'v,S,R,F> where
//     S: UserData,
//     R: Into<Value<'v>>,
//     F: Fn(VM<'v>, S) -> Result<R>{
//     getters: HashMap<String, F>,
//     setters: HashMap<String, F>,
//     phantom: PhantomData<&'v S>,
// }
// type Declos<'v, S> = dyn Fn(VM<'v>, S) -> Result<dyn Into<Value<'v>>>;

// type ToVal<'v> = dyn Into<Value<'v>>;
// type FU<'v,U> = fn(VM<'v>, U) -> Result< dyn Into<Value<'v>>>;
//
// pub struct UserDataFields<'v, U, F>
// where
//     U: UserData,
//     F: Fn(VM<'v>, U) -> Result<Value<'v>>,
// {
//     getters: HashMap<String, F>,
//     setters: HashMap<String, F>,
//     phantom: PhantomData<&'v F>,
//     phantom2: PhantomData<U>,
// }
// pub struct UserDataFields<'v>{
//     getters: HashMap<String,Box<Sized+Declos<'v,UserData>>>,
//     setters: HashMap<String, Declos<'v,UserData>>>,
// }

// impl<'v, U, F> UserDataFields<'v, U, F>
// where
//     U: UserData,
//     F: Fn(VM<'v>, U) -> Result<Value<'v>>,
// {
//     fn new() -> Self {
//         Self {
//             getters: HashMap::new(),
//             setters: HashMap::new(),
//             phantom: PhantomData,
//             phantom2: PhantomData,
//         }
//     }
//
//     fn add_field_method_get(&mut self, name: &str, closure: F) {}
//
//     fn add_field_method_set<T>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(VM<'v>, U, Value<'v>) -> Result<Value<'v>>,
//     {
//     }
// }

// impl<'v, S> UserDataFields<'v, S> {
//     fn add_field_method_get<F, R>(&mut self, name: &str, closure: F)
//     where
//         R: Into<Value<'v>>,
//         F: Fn(VM<'v>, S) -> Result<R>,
//     {
//         // self.getters.insert(name,closure);
//     }
// }

pub struct MethodsLookup<'v>(pub HashMap<&'v str, UserDataMethodsStruct>);
// pub type FieldsLookup<'v> = HashMap<&'v str, UserDataFieldMap<'v>>;
pub struct FieldsLookup(pub HashMap<TypeId, UserDataFieldsStruct>);

impl MethodsLookup<'_> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}
impl FieldsLookup {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

unsafe impl<'v> gc_arena::Collect for FieldsLookup {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }
}

unsafe impl<'v> gc_arena::Collect for MethodsLookup<'v> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }
}

pub(crate) fn build_userdata_maps<'v, U: UserData + 'static>(
    methods: &mut MethodsLookup,
    fields: &mut FieldsLookup,
) {
    let key = TypeId::of::<U>().clone();
    let mut f = UserDataFieldsStruct::new();
    U::add_fields(&mut f);

    let mut m = UserDataMethodsStruct::new();
    U::add_methods(&mut m);
    // methods.0.insert(key, m);
    fields.0.insert(key, f);
    // fields.insert(key, f);
}

// pub struct UserDataFields<'v,  F> where
// F: Fn(VM<'v>, dyn UserData) -> Result<Box<Value<'v>>>
// impl<'v, S> UserDataMethods<'v, M> for UserDataMethodMap<'v> {
//     fn add_method_mut<T, M, R>(&mut self, name: &str, closure: M)
//     where
//         T: Into<Value<'v>>,
//         R: Into<Value<'v>>,
//         M: Fn(VM<'v>, S, T) -> Result<R>,
//     {
//         // self.methods.insert(name, closure);
//     }
//
//     fn add_meta_method<T, M, R>(&mut self, name: &str, closure: M)
//     where
//         T: Into<Value<'v>>,
//         R: Into<Value<'v>>,
//         M: Fn(VM<'v>, S, T) -> Result<R>,
//     {
//     }
// }

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
    fn get_type() -> &'static str {
        "ent"
    }
    fn to_stringy(&self) -> String {
        "ent".to_string()
    }
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(methods: &mut M) {
        // let test=|_, this, _: ()| {
        //     Ok(format!("[entity {}]", this.get_id()))
        // };
        // let f: FU<Ent> = |_, this| {
        //     Ok(format!("[entity {}]", this.get_id()))
        // }
        let val = Value::Integer(5);
        //  : dyn Fn(Value)->i64

        let v = |val: i64| val * 4;
        let r = v(*val);
        methods.add_meta_method("__tostring", |vm, this: &Self, val: Value| {
            Ok(format!("[entity {}]", this.get_id()).into())
        });
        methods.add_meta_method("__concat", |_, this, _: Value| {
            Ok(format!("[entity {}]", this.get_id()).into())
        });
        // methods.add_method_mut("pos", |_, this, p: (f64, f64, f64)| {
        //     this.x = p.0;
        //     this.y = p.1;
        //     this.z = p.2;
        //
        //     Ok(())
        // });
    }

    fn add_fields<'v, F: UserDataFields<'v, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, this| Ok(this.x.into()));
        fields.add_field_method_set("x", |_, mut this, x: Value| {
            this.x = x.into();
            Ok(Value::Nil)
        });

        fields.add_field_method_get("y", |_, this| Ok(this.y.into()));
        fields.add_field_method_set("y", |_, mut this, y: Value| {
            this.y = y.into();
            Ok(Value::Nil)
        });

        fields.add_field_method_get("z", |_, this| Ok(this.z.into()));
        fields.add_field_method_set("z", |_, mut this, z: Value| {
            this.z = z.into();
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

// /// A specialized `Result` type used by `mlua`'s API.
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
