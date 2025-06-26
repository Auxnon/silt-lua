use std::{
    any::Any,
    collections::HashMap,
    marker::PhantomData,
    sync::{Arc, Mutex, Weak},
};

use gc_arena::{Collect, Gc, Mutation};

use crate::{code::OpCode, error::SiltError, lua::VM, value::Value};

/// Result type for Lua operations
pub type InnerResult<'gc> = Result<Value<'gc>, SiltError>;

/// Trait for Rust types that can be used as Lua UserData
pub trait UserData: Sized + 'static {
    /// Returns a unique type name for this UserData type
    fn type_name() -> &'static str;

    /// Register methods for this UserData type
    fn add_methods<'gc, M: UserDataMethods<'gc, Self>>(_: &mut M) {}

    /// Register fields for this UserData type
    fn add_fields<'gc, F: UserDataFields<'gc, Self>>(_: &mut F) {}

    /// Get a unique identifier for this UserData instance
    fn get_id(&self) -> usize;
}

/// Trait for registering methods on UserData types
pub trait UserDataMethods<'gc, T: UserData> {
    /// Add a metamethod to this UserData type
    fn add_meta_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static;

    /// Add a method that can mutate the UserData
    fn add_method_mut<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static;

    /// Add a method that doesn't mutate the UserData
    fn add_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static;
}

/// Trait for registering fields on UserData types
pub trait UserDataFields<'gc, T: UserData> {
    /// Add a field getter
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T) -> InnerResult<'gc> + 'static;

    /// Add a field setter
    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Value<'gc>) -> InnerResult<'gc> + 'static;
}

/// Type-erased function for calling a method on a UserData instance
pub type UserDataMethodFn<'gc, T> =
    Box<dyn Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Vec<Value<'gc>>) -> InnerResult<'gc>>;

/// Type-erased function for getting a field from a UserData instance
pub type UserDataGetterFn<'gc, T> = Box<dyn Fn(&VM<'gc>, &Mutation<'gc>, &T) -> InnerResult<'gc>>;

/// Type-erased function for setting a field on a UserData instance
pub type UserDataSetterFn<'gc, T> =
    Box<dyn Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Value<'gc>) -> InnerResult<'gc>>;

/// Trait object for type-erased UserData methods
pub trait UserDataMapTraitObj<'gc>: 'gc {
    fn call_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc>;
    fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc>;
    fn get_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &UserDataWrapper,
        name: &str,
    ) -> InnerResult<'gc>;
    fn set_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        value: Value<'gc>,
    ) -> InnerResult<'gc>;
}

/// Stores methods and fields for a specific UserData type T
pub struct UserDataTypedMap<'gc, T: UserData + 'static> {
    methods: HashMap<String, UserDataMethodFn<'gc, T>>,
    meta_methods: HashMap<String, UserDataMethodFn<'gc, T>>,
    getters: HashMap<String, UserDataGetterFn<'gc, T>>,
    setters: HashMap<String, UserDataSetterFn<'gc, T>>,
    _phantom: PhantomData<&'gc ()>,
}

impl<'gc, T: UserData + 'static> UserDataTypedMap<'gc, T> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            meta_methods: HashMap::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn to_string(&self) -> String {
        self.getters
            .keys()
            .map(|s| &**s)
            .collect::<Vec<&str>>()
            .join(",")
    }
}

impl<'gc, T: UserData + 'static> UserDataMapTraitObj<'gc> for UserDataTypedMap<'gc, T> {
    fn call_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        if let Some(method) = self.methods.get(name) {
            if let Ok(mut d) = ud.data.lock() {
                return match d.downcast_mut() {
                    Some(ud) => method(vm, mc, ud, args),
                    None => Err(SiltError::UDBadCast),
                };
            }
        };
        Err(SiltError::UDNoMethodRef)
    }

    fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        if let Some(method) = self.meta_methods.get(name) {
            if let Ok(mut d) = ud.data.lock() {
                return match d.downcast_mut() {
                    Some(ud) => method(vm, mc, ud, args),
                    None => Err(SiltError::UDBadCast),
                };
            }
        }
        Err(SiltError::UDNoMethodRef)
    }

    fn get_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &UserDataWrapper,
        name: &str,
    ) -> InnerResult<'gc> {
        // println!("field getters {}", name);
        // println!(
        //     "{}",
        //     self.getters
        //         .keys()
        //         .map(|s| &**s)
        //         .collect::<Vec<&str>>()
        //         .join(",")
        // );
        // let temp="NA".to_string();
        // let mut ar=self.getters.keys().map(|s|&**s);
        // ar.next();
        // ar.next();
        // let first= ar.next().unwrap_or(&temp);
        // println!("first {}",first);
        // println!("name {}",name);
        // println!("eq {}",first==name);
        if let Some(getter) = self.getters.get(name) {
            // println!("yeah we exist {}", name);
            if let Ok(d) = ud.data.lock() {
                return match d.downcast_ref() {
                    Some(ud) => getter(vm, mc, ud),
                    None => Err(SiltError::UDBadCast),
                };
            }
        }
        // println!("we dont exist");
        Err(SiltError::UDNoFieldGet)
    }

    fn set_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        value: Value<'gc>,
    ) -> InnerResult<'gc> {
        // println!("field setters");
        if let Some(setter) = self.setters.get(name) {
            if let Ok(mut d) = ud.data.lock() {
                return match d.downcast_mut() {
                    Some(ud) => setter(vm, mc, ud, value),
                    None => Err(SiltError::UDBadCast),
                };
            }
        }
        Err(SiltError::UDNoFieldSet)
    }
}

/// Type-erased container for UserData methods
pub struct UserDataMap<'gc> {
    data: Box<dyn UserDataMapTraitObj<'gc> + 'gc>,
}

impl<'gc> UserDataMap<'gc> {
    pub fn new<T: UserData + 'static>(methods: UserDataTypedMap<'gc, T>) -> Self {
        Self {
            data: Box::new(methods),
        }
    }

    pub fn call_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        self.data.call_method(vm, mc, ud, name, args)
    }

    pub fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        self.data.call_meta_method(vm, mc, ud, name, args)
    }

    pub fn get_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &UserDataWrapper,
        name: &str,
    ) -> InnerResult<'gc> {
        self.data.get_field(vm, mc, ud, name)
    }

    pub fn set_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        name: &str,
        value: Value<'gc>,
    ) -> InnerResult<'gc> {
        self.data.set_field(vm, mc, ud, name, value)
    }
}

// /// Implementation of UserDataMethods for registering methods
// pub struct UserDataMethodsImpl<'a, 'gc, T: UserData + 'static> {
//     methods: &'a mut UserDataMethods<'gc, T>,
// }
//
// impl<'a, 'gc, T: UserData + 'static> UserDataMethodsImpl<'a, 'gc, T> {
//     pub fn new(methods: &'a mut UserDataMethods<'gc, T>) -> Self {
//         Self { methods }
//     }
// }
//

impl<'a, 'gc, T: UserData + 'static> UserDataMethods<'gc, T> for UserDataTypedMap<'gc, T> {
    fn add_meta_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc, T> =
            Box::new(move |vm, mc, ud, vals| closure(vm, mc, ud, vals));
        self.meta_methods.insert(name.to_string(), func);
    }

    fn add_method_mut<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc, T> =
            Box::new(move |vm, mc, ud, vals| closure(vm, mc, ud, vals));
        self.methods.insert(name.to_string(), func);
    }

    fn add_method<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T, Vec<Value<'gc>>) -> InnerResult<'gc> + 'static,
    {
        let func: UserDataMethodFn<'gc, T> =
            Box::new(move |vm, mc, ud, vals| closure(vm, mc, ud, vals));
        self.methods.insert(name.to_string(), func);
    }
}

impl<'a, 'gc, T: UserData + 'static> UserDataFields<'gc, T> for UserDataTypedMap<'gc, T> {
    fn add_field_method_get<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T) -> InnerResult<'gc> + 'static,
    {
        println!("add getter {}", name);
        let func: UserDataGetterFn<'gc, T> = Box::new(move |vm, mc, ud| closure(vm, mc, ud));
        self.getters.insert(name.to_string(), func);
    }

    fn add_field_method_set<F>(&mut self, name: &str, closure: F)
    where
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Value<'gc>) -> InnerResult<'gc> + 'static,
    {
        println!("add setter {}", name);
        let func: UserDataSetterFn<'gc, T> =
            Box::new(move |vm, mc, ud, val| closure(vm, mc, ud, val));
        self.setters.insert(name.to_string(), func);
    }
}

/// Registry for UserData types
pub struct UserDataRegistry<'gc> {
    maps: HashMap<&'static str, UserDataMap<'gc>>,
    instance_data: HashMap<usize, &'static str>, // Maps instance ID to type_name
}

impl<'gc> UserDataRegistry<'gc> {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
            instance_data: HashMap::new(),
        }
    }

    /// Register a UserData type
    pub fn register<T: UserData>(&mut self) {
        let type_name = T::type_name();
        // Only register if not already registered
        if !self.maps.contains_key(type_name) {
            let mut typed_map = UserDataTypedMap::<T>::new();

            // Register methods
            T::add_methods(&mut typed_map);

            // Register fields
            T::add_fields(&mut typed_map);
            println!("TypedMap {}", typed_map.to_string());
            let map = UserDataMap::new(typed_map);

            // Store in registry
            self.maps.insert(type_name, map);
        }
    }

    /// Register a UserData instance
    pub fn register_instance(&mut self, wrapper: &UserDataWrapper) {
        self.instance_data.insert(wrapper.id(), wrapper.type_name());
    }

    /// Get methods for a UserData type
    pub fn get_map(&self, type_name: &'static str) -> Option<&UserDataMap<'gc>> {
        self.maps.get(type_name)
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
    data: Arc<Mutex<dyn Any>>,
    id: usize,
    type_name: &'static str,
    // Index in the VM's userdata_stack
    stack_index: Option<usize>,
}

pub struct WeakWrapper {
    data: Weak<Mutex<dyn Any>>,
    id: usize,
    type_name: &'static str,
    // Index in the VM's userdata_stack
    stack_index: Option<usize>,
}

impl WeakWrapper {
    /// Create a new WeakWrapper from a UserDataWrapper
    pub fn from_wrapper(wrapper: &UserDataWrapper) -> Self {
        Self {
            data: Arc::downgrade(&wrapper.data),
            id: wrapper.id,
            type_name: wrapper.type_name,
            stack_index: wrapper.stack_index,
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

    /// Try to upgrade to a full UserDataWrapper
    pub fn upgrade(&self) -> Option<UserDataWrapper> {
        self.data.upgrade().map(|data| UserDataWrapper {
            data,
            id: self.id,
            type_name: self.type_name,
            stack_index: self.stack_index,
        })
    }

    /// Check if the original UserDataWrapper has been dropped
    pub fn is_dropped(&self) -> bool {
        self.data.upgrade().is_none()
    }

    /// Convert to a string representation
    pub fn to_string(&self) -> String {
        format!("{} weak userdata (id: {})", self.type_name, self.id)
    }
}

impl UserDataWrapper {
    /// Create a new UserData wrapper
    pub fn new<T: UserData>(data: T) -> Self {
        let type_name = T::type_name();
        let id = data.get_id();
        Self {
            data: Arc::new(Mutex::new(data)),
            id,
            type_name,
            stack_index: None,
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
    // pub fn as_any(&self) -> &dyn Any {
    //     self.data.as_ref()
    // }

    /// Get a mutable reference to the wrapped data as Any
    // pub fn as_any_mut<'a: 'static>(&mut self) -> &'a mut dyn Any {
    //     &mut self.data.borrow_mut()
    // }

    /// Set the stack index for this UserData
    pub fn set_stack_index(&mut self, index: usize) {
        self.stack_index = Some(index);
    }

    /// Get the stack index for this UserData
    pub fn stack_index(&self) -> Option<usize> {
        self.stack_index
    }

    /// Convert to a string representation
    pub fn to_string(&self) -> String {
        format!("{} userdata (id: {})", self.type_name, self.id)
    }

    // pub fn downcast_mut<'a, 'b: 'a, T>(&'b mut self) -> &'a mut T {
    //     self.data.lock().unwrap().downcast_mut::<T>().unwrap()
    //     //.downcast_mut::<T>()
    // }
}

// impl Deref for UserDataWrapper {
//     type Target = dyn Any;
//
//     fn deref(&self) -> &Self::Target {
//         self.data.borrow()
//     }
// }

// impl DerefMut for UserDataWrapper {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         let v=self.data.borrow_mut().lock().unwrap().downcast_mut().unwrap();
//     }
// }

impl Clone for UserDataWrapper {
    fn clone(&self) -> Self {
        // This is a shallow clone - we're just cloning the Box pointer, not the data inside
        Self {
            data: self.data.clone(),
            id: self.id,
            type_name: self.type_name,
            stack_index: self.stack_index,
        }
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
pub struct TestEnt {
    x: f64,
    y: f64,
    z: f64,
}

impl TestEnt {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl UserData for TestEnt {
    fn type_name() -> &'static str {
        "ent"
    }

    fn get_id(&self) -> usize {
        0
    }

    fn add_methods<'gc, M: UserDataMethods<'gc, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, _, this, _| {
            Ok(Value::String(format!("[entity {}]", this.get_id())))
        });

        methods.add_meta_method("__concat", |_, _, this, _| {
            Ok(Value::String(format!("[entity {}]", this.get_id())))
        });

        methods.add_method_mut("pos", |_, _, this, args| {
            // Example of parsing a table to set position
            if let Some(Value::Table(t)) = args.first() {
                let t_ref = (*t).borrow();
                if let Some(Value::Number(x)) = t_ref.get(&Value::String("x".to_string())) {
                    this.x = *x;
                }
                if let Some(Value::Number(y)) = t_ref.get(&Value::String("y".to_string())) {
                    this.y = *y;
                }
                if let Some(Value::Number(z)) = t_ref.get(&Value::String("z".to_string())) {
                    this.z = *z;
                }
            }
            Ok(Value::Nil)
        });

        // Add a method that demonstrates multiple parameters
        methods.add_method_mut("set_pos", |vm, mc, this, args| {
            if args.len() >= 3 {
                if let Value::Number(x) = args[0] {
                    this.x = x;
                } else if let Value::Integer(x) = args[0] {
                    this.x = x as f64;
                }

                if let Value::Number(y) = args[1] {
                    this.y = y;
                } else if let Value::Integer(y) = args[1] {
                    this.y = y as f64;
                }

                if let Value::Number(z) = args[2] {
                    this.z = z;
                } else if let Value::Integer(z) = args[2] {
                    this.z = z as f64;
                }
            }

            // Return multiple values as an example
            let mut vals = vm.raw_table();
            vals.push(Value::Number(this.x));
            vals.push(Value::Number(this.y));
            vals.push(Value::Number(this.z));

            Ok(vm.wrap_table(mc, vals))
            // Ok(Value::Nil)
        });
    }

    fn add_fields<'gc, F: UserDataFields<'gc, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, _, this| Ok(Value::Number(this.x)));

        fields.add_field_method_set("x", |_, _, this, val| {
            if let Value::Number(x) = val {
                this.x = x;
            } else if let Value::Integer(x) = val {
                this.x = x as f64;
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("y", |_, _, this| Ok(Value::Number(this.y)));

        fields.add_field_method_set("y", |_, _, this, val| {
            if let Value::Number(y) = val {
                this.y = y;
            } else if let Value::Integer(y) = val {
                this.y = y as f64;
            }
            Ok(Value::Nil)
        });

        fields.add_field_method_get("z", |_, _, this| Ok(Value::Number(this.z)));

        fields.add_field_method_set("z", |_, _, this, val| {
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
    pub fn as_table_key(&self) -> &'static str {
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
    use gc_arena::lock::RefLock;

    use super::*;
    use crate::lua::{UDVec, VM};

    /// Create a new UserData value
    pub fn create_userdata<'gc, T: UserData>(
        reg: &mut UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        data: T,
        userdata_stack: &mut Option<UDVec>,
    ) -> Value<'gc> {
        // Register the type if it hasn't been registered yet
        let type_name = T::type_name();
        if !reg.maps.contains_key(type_name) {
            reg.register::<T>();
        }

        // Create the UserData wrapper
        let mut wrapper = UserDataWrapper::new(data);

        // Add to the userdata_stack and get the index
        if let Some(stack) = userdata_stack {
            let index = stack.0.len();
            wrapper.set_stack_index(index);

            // Create a weak wrapper and store it in the stack
            let weak_wrapper = WeakWrapper::from_wrapper(&wrapper);
            stack.0.push(weak_wrapper);
        };

        // Create the GC-managed wrapper
        let ud_gc = Gc::new(mc, RefLock::new(wrapper));

        // Set the stack index in the GC-managed wrapper
        // ud_gc.borrow_mut(mc).set_stack_index(index);

        // Return as a Value
        Value::UserData(ud_gc)
    }

    /// Call a method on a UserData value
    pub fn call_method<'gc>(
        vm: &VM<'gc>,
        reg: &UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        userdata: &mut UserDataWrapper,
        method_name: &str,
        args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the method in the registry
        if let Some(map) = reg.get_map(type_name) {
            return map.call_method(vm, mc, userdata, method_name, args);
        }

        Err(SiltError::UDNoMap)
    }

    /// Call a metamethod on a UserData value
    pub fn call_meta_method<'gc>(
        vm: &VM<'gc>,
        reg: &UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        userdata: &mut UserDataWrapper,
        meta_method: MetaMethod,
        args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();
        let meta_key = meta_method.as_table_key();

        // Look up the metamethod in the registry
        if let Some(map) = reg.get_map(type_name) {
            return map.call_meta_method(vm, mc, userdata, meta_key, args);
        }

        Err(SiltError::UDNoMap)
    }

    /// Get a field from a UserData value
    pub fn get_field<'gc>(
        vm: &VM<'gc>,
        reg: &UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        userdata: &mut UserDataWrapper,
        field_name: &str,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the field getter in the registry
        if let Some(map) = reg.get_map(type_name) {
            return map.get_field(vm, mc, userdata, field_name);
        }

        Err(SiltError::UDNoMap)
    }

    /// Set a field on a UserData value
    pub fn set_field<'gc>(
        vm: &VM<'gc>,
        reg: &UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        userdata: &mut UserDataWrapper,
        field_name: &str,
        value: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let type_name = userdata.type_name();

        // Look up the field setter in the registry
        if let Some(map) = reg.get_map(type_name) {
            return map.set_field(vm, mc, userdata, field_name, value);
        }

        Err(SiltError::UDNoMap)
    }
}
