use std::{any::TypeId, collections::HashMap, marker::PhantomData};

use gc_arena::Collect;

use crate::{code::OpCode, error::SiltError, prelude::VM, value::Value};

/// Result type for Lua operations
pub type LuaResult<'v> = Result<Value<'v>, SiltError>;

/// Trait for Rust types that can be used as Lua UserData
pub trait UserData: Sized {
    /// Returns a unique type name for this UserData type
    fn get_type() -> &'static str
    where
        Self: Sized;

    /// Register methods for this UserData type
    fn add_methods<'v, M: UserDataMethods<'v, Self>>(methods: &mut M)
    where
        Self: Sized,
    {
    }

    /// Register fields for this UserData type
    fn add_fields<'v, F: UserDataFields<'v, Self>>(fields: &mut F)
    where
        Self: Sized,
    {
    }

    /// Get a unique identifier for this UserData instance
    fn get_id(&self) -> usize;
}

// trait IV<'v> = Into<Value<'v>>;
// type ValueResult = Result<Value<'v>>
// trait CC =  Fn(VM<'v>, S, impl Into<Value<'v>>) -> LuaResult<'v>
// type Closer = impl Fn(VM<'v>, S, impl Into<Value<'v>>) -> LuaResult<'v>
// type C =  impl Fn(VM<'v>, S, Value<'v>) -> Result<Value<'v>>

/// Trait for registering methods on UserData types
pub trait UserDataMethods<'v, T: UserData> {
    /// Add a metamethod to this UserData type
    fn add_meta_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'v>, &T, Value<'v>) -> LuaResult<'v> + 'static;

    /// Add a method that can mutate the UserData
    fn add_method_mut<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'v>, &mut T, Value<'v>) -> LuaResult<'v> + 'static;

    /// Add a method that doesn't mutate the UserData
    fn add_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'v>, &T, Value<'v>) -> LuaResult<'v> + 'static;
}

/// Trait for registering fields on UserData types
pub trait UserDataFields<'v, T: UserData> {
    /// Add a field getter
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'v>, &T) -> LuaResult<'v>;

    /// Add a field setter
    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'v>, &mut T, Value<'v>) -> LuaResult<'v>;
}

/// Type-erased function for calling a method on a UserData instance
pub type UserDataMethodFn<'v, U: UserData> =
    Box<dyn Fn(&mut VM<'v>, &mut U, Value<'v>) -> Result<Value<'v>, SiltError>>;

/// Type-erased function for getting a field from a UserData instance
pub type UserDataGetterFn<'v, U> = Box<dyn Fn(&mut VM<'v>, &U) -> Result<Value<'v>, SiltError>>;

/// Type-erased function for setting a field on a UserData instance
pub type UserDataSetterFn<'v, U> =
    Box<dyn Fn(&mut VM<'v>, &mut U, Value<'v>) -> Result<Value<'v>, SiltError>>;

/// Stores methods for a UserData type
#[derive(Default)]
pub struct UserDataMap<'v, U: UserData> {
    methods: HashMap<String, UserDataMethodFn<'v, U>>,
    meta_methods: HashMap<String, UserDataMethodFn<'v, U>>,
    pub getters: HashMap<String, UserDataGetterFn<'v, U>>,
    pub setters: HashMap<String, UserDataSetterFn<'v, U>>,
}

pub trait UserDataTemplate<'v >: Sized {
    // fn get_method(&self, name: &str) -> Option<&UserDataMethodFn<'v, U>>;
    // fn get_meta_method(&self, name: &str) -> Option<&UserDataMethodFn<'v, U>>;
}

impl<'v, U: UserData> UserDataMap<'v, U> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            meta_methods: HashMap::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
        }
    }
}

// impl<'v, U: UserData> UserDataTemplate<'v, U> for UserDataMap<'v, U> {
//     // fn get_method(&self, name: &str) -> Option<&UserDataMethodFn<'v, U>> {
//     //     self.methods.get(name)
//     // }
//     //
//     // fn get_meta_method(&self, name: &str) -> Option<&UserDataMethodFn<'v, U>> {
//     //     self.meta_methods.get(name)
//     // }
// }

/// Implementation of UserDataMethods for registering methods
// pub struct UserDataMethodsImpl<'v, U: UserData> {
//     methods: &'v mut UserDataMethodsMap<'v,U>  ,
//     _phantom: PhantomData<U>,
// }

// impl<'v, U: UserData> UserDataMethodsImpl<'v, U> {
//     pub fn new(methods: &'v mut UserDataMethodsMap<'v, U>) -> Self {
//         Self {
//             methods,
//             _phantom: PhantomData,
//         }
//     }
// }

// impl<'v, U: UserData> UserDataMethods<'v, U> for UserDataMethodsImpl<'v, U> {
//     fn add_meta_method<F>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(&mut VM<'v>, &U, Value<'v>) -> LuaResult<'v> + 'static,
//     {
//         let func: UserDataMethodFn<'v, U> = Box::new(move |vm, ud, val| {
//             // We can't use Any::downcast_ref anymore since T isn't 'static
//             // Instead, we use a type-erased approach with a closure that captures T
//             let typed_ud = unsafe { &*(ud as *const dyn UserData as *const U) };
//             closure(vm, typed_ud, val)
//         });
//         self.methods.meta_methods.insert(name.to_string(), func);
//     }
//
//     fn add_method_mut<F>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(&mut VM<'v>, &mut U, Value<'v>) -> LuaResult<'v> + 'static,
//     {
//         let func: UserDataMethodFn<'v> = Box::new(move |vm, ud, val| {
//             // We can't use Any::downcast_mut anymore since T isn't 'static
//             // Instead, we use a type-erased approach with a closure that captures T
//             let typed_ud = unsafe { &mut *(ud as *mut dyn UserData as *mut U) };
//             closure(vm, typed_ud, val)
//         });
//         self.methods.methods.insert(name.to_string(), func);
//     }
//
//     fn add_method<F>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(&mut VM<'v>, &U, Value<'v>) -> LuaResult<'v> + 'static,
//     {
//         let func: UserDataMethodFn<'v> = Box::new(move |vm, ud, val| {
//             // We can't use Any::downcast_ref anymore since T isn't 'static
//             // Instead, we use a type-erased approach with a closure that captures T
//             let typed_ud = unsafe { &*(ud as *const dyn UserData as *const U) };
//             closure(vm, typed_ud, val)
//         });
//         self.methods.methods.insert(name.to_string(), func);
//     }
// }

/// Implementation of UserDataFields for registering fields
// pub struct UserDataFieldsImpl<'v, T: UserData> {
//     fields: &'v mut UserDataFieldsMap,
//     _phantom: PhantomData<T>,
// }
//
// impl<'v, T: UserData> UserDataFieldsImpl<'v, T> {
//     pub fn new(fields: &'v mut UserDataFieldsMap) -> Self {
//         Self {
//             fields,
//             _phantom: PhantomData,
//         }
//     }
// }

// impl<'v, T: UserData> UserDataFields<'v, T> for UserDataFieldsImpl<'v, T> {
//     fn add_field_method_get<F>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(&mut VM<'v>, &T) -> LuaResult<'v> + 'static,
//     {
//         let func: UserDataGetterFn<'v> = Box::new(move |vm, ud| {
//             // We can't use Any::downcast_ref anymore since T isn't 'static
//             // Instead, we use a type-erased approach with a closure that captures T
//             let typed_ud = unsafe { &*(ud as *const dyn UserData as *const T) };
//             closure(vm, typed_ud)
//         });
//         self.fields.getters.insert(name.to_string(), func);
//     }
//
//     fn add_field_method_set<F>(&mut self, name: &str, closure: F)
//     where
//         F: Fn(&mut VM<'v>, &mut T, Value<'v>) -> LuaResult<'v> + 'static,
//     {
//         let func: UserDataSetterFn<'v> = Box::new(move |vm, ud, val| {
//             // We can't use Any::downcast_mut anymore since T isn't 'static
//             // Instead, we use a type-erased approach with a closure that captures T
//             let typed_ud = unsafe { &mut *(ud as *mut dyn UserData as *mut T) };
//             closure(vm, typed_ud, val)
//         });
//         self.fields.setters.insert(name.to_string(), func);
//     }
// }
//
/// Registry for UserData types
pub struct UserDataRegistry<'v> {
    maps: HashMap<&'static str, Box<UserDataMap<UserData>>>,
    // fields: HashMap<&'static str, UserDataFieldsMap<'v, U>>,
    // instance_data: HashMap<usize, (&'static str, usize)>, // Maps instance ID to (type_name, type_id)
    // next_type_id: usize,
}

impl<'gc, U: UserData> UserDataRegistry<'gc, U> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            fields: HashMap::new(),
            instance_data: HashMap::new(),
            next_type_id: 1,
        }
    }

    /// Register a UserData type
    pub fn register<T: UserData>(&mut self) {
        let type_name = T::get_type();

        // Only register if not already registered
        if !self.methods.contains_key(type_name) {
            // Register methods
            let mut methods_map = UserDataMethodsMap::new();
            let mut methods_impl = UserDataMethodsImpl::<T>::new(&mut methods_map);
            T::add_methods(&mut methods_impl);
            self.methods.insert(type_name, methods_map);

            // Register fields
            let mut fields_map = UserDataFieldsMap::new();
            let mut fields_impl = UserDataFieldsImpl::<T>::new(&mut fields_map);
            T::add_fields(&mut fields_impl);
            self.fields.insert(type_name, fields_map);

            self.next_type_id += 1;
        }
    }

    /// Register a UserData instance
    pub fn register_instance(&mut self, wrapper: &UserDataWrapper) {
        self.instance_data
            .insert(wrapper.id(), (wrapper.type_name(), 0));
    }

    /// Get methods for a UserData type
    pub fn get_methods(&self, type_name: &'static str) -> Option<&UserDataMethodsMap<'gc>> {
        self.methods.get(type_name)
    }

    /// Get fields for a UserData type
    pub fn get_fields(&self, type_name: &'static str) -> Option<&UserDataFieldsMap<'gc>> {
        self.fields.get(type_name)
    }

    /// Get type name for a UserData instance
    pub fn get_type_for_instance(&self, id: usize) -> Option<&'static str> {
        self.instance_data.get(&id).map(|(type_name, _)| *type_name)
    }
}

unsafe impl Collect for UserDataRegistry {
    fn needs_trace() -> bool {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {
        // No GC references to trace
    }
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

use std::any::Any;

/// A wrapper for UserData objects
pub struct UserDataWrapper {
    data: Box<dyn UserData>,
    id: usize,
    type_name: &'static str,
}

impl UserDataWrapper {
    /// Create a new UserData wrapper
    pub fn new<T: UserData>(data: T) -> Self {
        let type_name = T::get_type();
        let id = data.get_id();
        Self {
            data: Box::new(data),
            id,
            type_name,
        }
    }

    /// Get the unique ID of the wrapped UserData
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the type name of the wrapped UserData
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Get a reference to the wrapped data
    pub fn as_ref(&self) -> &dyn UserData {
        self.data.as_ref()
    }

    /// Get a mutable reference to the wrapped data
    pub fn as_mut(&mut self) -> &mut dyn UserData {
        self.data.as_mut()
    }

    /// Convert to a string representation
    pub fn to_string(&self) -> String {
        format!("{} userdata (id: {})", self.type_name, self.id)
    }
}

unsafe impl Collect for UserDataWrapper {
    fn needs_trace() -> bool {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {
        // No GC references to trace
    }
}

/// Example UserData implementation
pub struct Ent {
    x: f64,
    y: f64,
    z: f64,
}

impl Ent {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn entity_id(&self) -> i64 {
        0
    }
}

impl UserData for Ent {
    fn get_type() -> &'static str {
        "ent"
    }

    fn get_id(&self) -> usize {
        // Use a combination of memory address and values for uniqueness
        let ptr = self as *const Self as usize;
        ptr ^ ((self.x.to_bits() as usize) << 32)
            ^ (self.y.to_bits() as usize)
            ^ (self.z.to_bits() as usize)
    }

    fn add_methods<'v, M: UserDataMethods<'v, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, _| {
            Ok(Value::String(Box::new(format!(
                "[entity {}]",
                this.get_id()
            ))))
        });

        methods.add_meta_method("__concat", |_, this, _| {
            Ok(Value::String(Box::new(format!(
                "[entity {}]",
                this.get_id()
            ))))
        });

        methods.add_method_mut("pos", |_, this, val| {
            // Example of parsing a table to set position
            if let Value::Table(t) = val {
                let t_ref = t.borrow();
                if let Some(Value::Number(x)) = t_ref.get(&Value::String(Box::new("x".to_string())))
                {
                    this.x = *x;
                }
                if let Some(Value::Number(y)) = t_ref.get(&Value::String(Box::new("y".to_string())))
                {
                    this.y = *y;
                }
                if let Some(Value::Number(z)) = t_ref.get(&Value::String(Box::new("z".to_string())))
                {
                    this.z = *z;
                }
            }
            Ok(Value::Nil)
        });
    }

    fn add_fields<'v, F: UserDataFields<'v, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, this| Ok(Value::Number(this.x)));

        fields.add_field_method_set("x", |_, this, val| {
            if let Value::Number(x) = val {
                this.x = x;
            } else if let Value::Integer(x) = val {
                this.x = x as f64;
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("y", |_, this| Ok(Value::Number(this.y)));

        fields.add_field_method_set("y", |_, this, val| {
            if let Value::Number(y) = val {
                this.y = y;
            } else if let Value::Integer(y) = val {
                this.y = y as f64;
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("z", |_, this| Ok(Value::Number(this.z)));

        fields.add_field_method_set("z", |_, this, val| {
            if let Value::Number(z) = val {
                this.z = z;
            } else if let Value::Integer(z) = val {
                this.z = z as f64;
            }
            Ok(Value::Nil)
        });
    }
}

/// Metamethods that can be implemented by UserData types
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

impl From<OpCode> for MetaMethod {
    fn from(op: OpCode) -> Self {
        match op {
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

/// Helper functions for VM integration
pub mod vm_integration {
    use super::*;
    use crate::error::SiltError;
    use crate::lua::VM;
    use crate::value::Value;
    use gc_arena::{Gc, Mutation};

    /// Create a new UserData value
    pub fn create_userdata<'gc, T: UserData>(
        vm: &mut VM<'gc>,
        mc: &Mutation<'gc>,
        data: T,
    ) -> Value<'gc> {
        // Register the type if it hasn't been registered yet
        let type_name = T::get_type();
        if !vm.userdata_registry.methods.contains_key(type_name) {
            vm.userdata_registry.register::<T>();
        }

        // Create the UserData wrapper and return it as a Value
        let wrapper = UserDataWrapper::new(data);
        let ud_gc = Gc::new(mc, wrapper);

        // Register the instance
        vm.userdata_registry.register_instance(&ud_gc);

        Value::UserData(ud_gc)
    }

    /// Call a method on a UserData value
    pub fn call_method<'gc>(
        vm: &mut VM<'gc>,
        reg: &UserDataRegistry<'gc>,
        userdata: &mut UserDataWrapper,
        method_name: &str,
        arg: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the method in the registry
        if let Some(methods) = reg.get_methods(type_name) {
            if let Some(method) = methods.get_method(method_name) {
                // Call the method with the userdata and argument
                return method(vm, userdata, arg);
            }
        }

        Err(SiltError::UDNoMethodRef)
    }

    /// Call a metamethod on a UserData value
    pub fn call_meta_method<'gc>(
        vm: &mut VM<'gc>,
        userdata: &Gc<'gc, UserDataWrapper>,
        meta_method: MetaMethod,
        arg: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();
        let meta_key = meta_method.to_table_key();

        // Look up the metamethod in the registry
        if let Some(methods) = vm.userdata_registry.get_methods(type_name) {
            if let Some(method) = methods.get_meta_method(meta_key) {
                // Call the metamethod with the userdata and argument
                return method(vm, userdata.as_mut(), arg);
            }
        }

        Err(SiltError::MetaMethodMissing(meta_method))
    }

    /// Get a field from a UserData value
    pub fn get_field<'gc>(
        vm: &mut VM<'gc>,
        userdata: &Gc<'gc, UserDataWrapper>,
        field_name: &str,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the field getter in the registry
        if let Some(fields) = vm.userdata_registry.get_fields(type_name) {
            if let Some(getter) = fields.getters.get(field_name) {
                // Call the getter with the userdata
                return getter(vm, userdata.as_ref());
            }
        }

        Err(SiltError::UDNoFieldRef)
    }

    /// Set a field on a UserData value
    pub fn set_field<'gc>(
        vm: &mut VM<'gc>,
        userdata: &Gc<'gc, UserDataWrapper>,
        field_name: &str,
        value: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the field setter in the registry
        if let Some(fields) = vm.userdata_registry.get_fields(type_name) {
            if let Some(setter) = fields.setters.get(field_name) {
                // Call the setter with the userdata and value
                return setter(vm, userdata.as_mut(), value);
            }
        }

        Err(SiltError::UDNoFieldRef)
    }
}
