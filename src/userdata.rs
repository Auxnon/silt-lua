use std::{any::Any, collections::HashMap, marker::PhantomData};

use gc_arena::{Collect, Gc, Mutation};

use crate::{code::OpCode, error::SiltError, lua::VM, value::Value};

/// Result type for Lua operations
pub type LuaResult<'gc> = Result<Value<'gc>, SiltError>;

/// Trait for Rust types that can be used as Lua UserData
pub trait UserData: 'static {
    /// Returns a unique type name for this UserData type
    fn type_name() -> &'static str;

    /// Register methods for this UserData type
    fn add_methods<'gc, M: UserDataMethods<'gc, Self>>(methods: &mut M) {}

    /// Register fields for this UserData type
    fn add_fields<'gc, F: UserDataFields<'gc, Self>>(fields: &mut F) {}

    /// Get a unique identifier for this UserData instance
    fn get_id(&self) -> usize;
}

/// Trait for registering methods on UserData types
pub trait UserDataMethods<'gc, T: UserData> {
    /// Add a metamethod to this UserData type
    fn add_meta_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T, Value<'gc>) -> LuaResult<'gc> + 'static;

    /// Add a method that can mutate the UserData
    fn add_method_mut<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &mut T, Value<'gc>) -> LuaResult<'gc> + 'static;

    /// Add a method that doesn't mutate the UserData
    fn add_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T, Value<'gc>) -> LuaResult<'gc> + 'static;
}

/// Trait for registering fields on UserData types
pub trait UserDataFields<'gc, T: UserData> {
    /// Add a field getter
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T) -> LuaResult<'gc> + 'static;

    /// Add a field setter
    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &mut T, Value<'gc>) -> LuaResult<'gc> + 'static;
}

/// Type-erased function for calling a method on a UserData instance
pub type UserDataMethodFn<'gc> = Box<dyn Fn(&mut VM<'gc>, &mut dyn Any, Value<'gc>) -> LuaResult<'gc>>;

/// Type-erased function for getting a field from a UserData instance
pub type UserDataGetterFn<'gc> = Box<dyn Fn(&mut VM<'gc>, &dyn Any) -> LuaResult<'gc>>;

/// Type-erased function for setting a field on a UserData instance
pub type UserDataSetterFn<'gc> = Box<dyn Fn(&mut VM<'gc>, &mut dyn Any, Value<'gc>) -> LuaResult<'gc>>;

/// Stores methods and fields for a UserData type
pub struct UserDataMethods<'gc> {
    methods: HashMap<String, UserDataMethodFn<'gc>>,
    meta_methods: HashMap<String, UserDataMethodFn<'gc>>,
    getters: HashMap<String, UserDataGetterFn<'gc>>,
    setters: HashMap<String, UserDataSetterFn<'gc>>,
}

impl<'gc> UserDataMethods<'gc> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            meta_methods: HashMap::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
        }
    }

    pub fn get_method(&self, name: &str) -> Option<&UserDataMethodFn<'gc>> {
        self.methods.get(name)
    }

    pub fn get_meta_method(&self, name: &str) -> Option<&UserDataMethodFn<'gc>> {
        self.meta_methods.get(name)
    }

    pub fn get_getter(&self, name: &str) -> Option<&UserDataGetterFn<'gc>> {
        self.getters.get(name)
    }

    pub fn get_setter(&self, name: &str) -> Option<&UserDataSetterFn<'gc>> {
        self.setters.get(name)
    }
}

/// Implementation of UserDataMethods for registering methods
pub struct UserDataMethodsImpl<'a, 'gc, T: UserData> {
    methods: &'a mut UserDataMethods<'gc>,
    _phantom: PhantomData<T>,
}

impl<'a, 'gc, T: UserData> UserDataMethodsImpl<'a, 'gc, T> {
    pub fn new(methods: &'a mut UserDataMethods<'gc>) -> Self {
        Self {
            methods,
            _phantom: PhantomData,
        }
    }
}

impl<'a, 'gc, T: UserData> UserDataMethods<'gc, T> for UserDataMethodsImpl<'a, 'gc, T> {
    fn add_meta_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T, Value<'gc>) -> LuaResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc> = Box::new(move |vm, ud, val| {
            let typed_ud = ud.downcast_ref::<T>().unwrap();
            closure(vm, typed_ud, val)
        });
        self.methods.meta_methods.insert(name.to_string(), func);
    }

    fn add_method_mut<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &mut T, Value<'gc>) -> LuaResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc> = Box::new(move |vm, ud, val| {
            let typed_ud = ud.downcast_mut::<T>().unwrap();
            closure(vm, typed_ud, val)
        });
        self.methods.methods.insert(name.to_string(), func);
    }

    fn add_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T, Value<'gc>) -> LuaResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc> = Box::new(move |vm, ud, val| {
            let typed_ud = ud.downcast_ref::<T>().unwrap();
            closure(vm, typed_ud, val)
        });
        self.methods.methods.insert(name.to_string(), func);
    }
}

/// Implementation of UserDataFields for registering fields
pub struct UserDataFieldsImpl<'a, 'gc, T: UserData> {
    methods: &'a mut UserDataMethods<'gc>,
    _phantom: PhantomData<T>,
}

impl<'a, 'gc, T: UserData> UserDataFieldsImpl<'a, 'gc, T> {
    pub fn new(methods: &'a mut UserDataMethods<'gc>) -> Self {
        Self {
            methods,
            _phantom: PhantomData,
        }
    }
}

impl<'a, 'gc, T: UserData> UserDataFields<'gc, T> for UserDataFieldsImpl<'a, 'gc, T> {
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &T) -> LuaResult<'gc> + 'static,
    {
        let func: UserDataGetterFn<'gc> = Box::new(move |vm, ud| {
            let typed_ud = ud.downcast_ref::<T>().unwrap();
            closure(vm, typed_ud)
        });
        self.methods.getters.insert(name.to_string(), func);
    }

    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&mut VM<'gc>, &mut T, Value<'gc>) -> LuaResult<'gc> + 'static,
    {
        let func: UserDataSetterFn<'gc> = Box::new(move |vm, ud, val| {
            let typed_ud = ud.downcast_mut::<T>().unwrap();
            closure(vm, typed_ud, val)
        });
        self.methods.setters.insert(name.to_string(), func);
    }
}

/// Registry for UserData types
pub struct UserDataRegistry<'gc> {
    methods: HashMap<&'static str, UserDataMethods<'gc>>,
    instance_data: HashMap<usize, &'static str>, // Maps instance ID to type_name
}

impl<'gc> UserDataRegistry<'gc> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            instance_data: HashMap::new(),
        }
    }

    /// Register a UserData type
    pub fn register<T: UserData>(&mut self) {
        let type_name = T::type_name();

        // Only register if not already registered
        if !self.methods.contains_key(type_name) {
            // Create methods container
            let mut methods = UserDataMethods::new();
            
            // Register methods
            let mut methods_impl = UserDataMethodsImpl::<T>::new(&mut methods);
            T::add_methods(&mut methods_impl);
            
            // Register fields
            let mut fields_impl = UserDataFieldsImpl::<T>::new(&mut methods);
            T::add_fields(&mut fields_impl);
            
            // Store in registry
            self.methods.insert(type_name, methods);
        }
    }

    /// Register a UserData instance
    pub fn register_instance(&mut self, wrapper: &UserDataWrapper) {
        self.instance_data.insert(wrapper.id(), wrapper.type_name());
    }

    /// Get methods for a UserData type
    pub fn get_methods(&self, type_name: &'static str) -> Option<&UserDataMethods<'gc>> {
        self.methods.get(type_name)
    }

    /// Get type name for a UserData instance
    pub fn get_type_for_instance(&self, id: usize) -> Option<&'static str> {
        self.instance_data.get(&id).copied()
    }
}

unsafe impl<'gc> Collect for UserDataRegistry<'gc> {
    fn needs_trace() -> bool {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {
        // No GC references to trace
    }
}

/// A wrapper for UserData objects
pub struct UserDataWrapper {
    data: Box<dyn Any>,
    id: usize,
    type_name: &'static str,
}

impl UserDataWrapper {
    /// Create a new UserData wrapper
    pub fn new<T: UserData>(data: T) -> Self {
        let type_name = T::type_name();
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

    /// Get a reference to the wrapped data as Any
    pub fn as_any(&self) -> &dyn Any {
        self.data.as_ref()
    }

    /// Get a mutable reference to the wrapped data as Any
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
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
    fn type_name() -> &'static str {
        "ent"
    }

    fn get_id(&self) -> usize {
        // Use a combination of memory address and values for uniqueness
        let ptr = self as *const Self as usize;
        ptr ^ ((self.x.to_bits() as usize) << 32)
            ^ (self.y.to_bits() as usize)
            ^ (self.z.to_bits() as usize)
    }

    fn add_methods<'gc, M: UserDataMethods<'gc, Self>>(methods: &mut M) {
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

    fn add_fields<'gc, F: UserDataFields<'gc, Self>>(fields: &mut F) {
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

    /// Create a new UserData value
    pub fn create_userdata<'gc, T: UserData>(
        vm: &mut VM<'gc>,
        mc: &Mutation<'gc>,
        data: T,
    ) -> Value<'gc> {
        // Register the type if it hasn't been registered yet
        let type_name = T::type_name();
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
        userdata: &Gc<'gc, UserDataWrapper>,
        method_name: &str,
        arg: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the method in the registry
        if let Some(methods) = vm.userdata_registry.get_methods(type_name) {
            if let Some(method) = methods.get_method(method_name) {
                // Call the method with the userdata and argument
                return method(vm, userdata.as_any_mut(), arg);
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
                return method(vm, userdata.as_any_mut(), arg);
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
        if let Some(methods) = vm.userdata_registry.get_methods(type_name) {
            if let Some(getter) = methods.get_getter(field_name) {
                // Call the getter with the userdata
                return getter(vm, userdata.as_any());
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
        if let Some(methods) = vm.userdata_registry.get_methods(type_name) {
            if let Some(setter) = methods.get_setter(field_name) {
                // Call the setter with the userdata and value
                return setter(vm, userdata.as_any_mut(), value);
            }
        }

        Err(SiltError::UDNoFieldRef)
    }
}
