use core::f64;
use std::{
    any::Any,
    collections::HashMap,
    error::Error,
    marker::PhantomData,
    ops::Deref,
    rc::Rc,
    sync::{Arc, Mutex, Weak},
};

// use colored::Colorize;
use gc_arena::{Collect, Gc, Mutation};

use crate::{
    code::OpCode,
    error::SiltError,
    function::{NativeFunctionRaw, WrappedFn},
    lua::VM,
    value::{FromLua, FromLuaMulti, ToLua, Value, ValueRef, Variadic},
};

/// Result type for Lua operations
pub type InnerResult<'gc> = Result<Value<'gc>, SiltError>;
pub type ToInnerResult<'gc, V: ToLua<'gc>> = V;

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

// Simplified method signature that avoids complex lifetime issues
type UserDataMethodClosure<'gc> =
    Box<dyn Fn(&mut VM<'gc>, &Mutation<'gc>, &[Value<'gc>]) -> InnerResult<'gc> + 'gc>;

pub trait MethodHandler<'gc, T, A, R> {
    fn call_method<'a>(
        &self,
        vm: &mut VM<'gc>,
        mc: &Mutation<'gc>,
        userdata: Option<&mut T>,
        args: &'a [Value<'gc>],
    ) -> Result<R, SiltError>;
}

impl<'gc, T, A, R, F> MethodHandler<'gc, T, A, R> for F
where
    A: FromLuaMulti<'gc>,
    R: ToLua<'gc>,
    // We bind the closure here.
    // Note: If you still get static errors here, see step 4 below.
    F: Fn(
            &mut VM<'gc>,
            &Mutation<'gc>,
            Option<&mut T>,
            A,
            // <A as FromLuaMulti<'gc>>::Output,
        ) -> Result<R, SiltError>
        + 'gc,
{
    fn call_method<'a>(
        &self,
        vm: &mut VM<'gc>,
        mc: &Mutation<'gc>,
        userdata: Option<&mut T>,
        args: &'a [Value<'gc>],
    ) -> Result<R, SiltError> {
        // 1. Perform conversion using the lifetime 'a from the arguments
        let converted_args = A::from_lua_multi(args, vm, mc)?;

        // 2. Call the closure
        (self)(vm, mc, userdata, converted_args)
    }
}

/// Trait for registering methods on UserData types
pub trait UserDataMethods<'gc, T: UserData> {
    /// Add a metamethod to this UserData type
    fn add_meta_method<M, A, R, F>(&mut self, name: M, closure: F)
    where
        M: Into<MetaMethod>,
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, Option<&mut T>, A) -> Result<R, SiltError> + 'gc;
    /// Add a method that can mutate the UserData
    fn add_method_mut<A, R, F>(&mut self, name: &str, closure: F)
    where
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        F: Fn(
                &mut VM<'gc>,
                &Mutation<'gc>,
                Option<&mut T>,
                A, // <A as FromLuaMulti<'gc>>::Output,
            ) -> Result<R, SiltError>
            + 'gc;
    // F: MethodHandler<'gc, T, A, R> + 'gc;

    /// Add a method that doesn't mutate the UserData
    fn add_method<A, R, F>(&mut self, name: &str, closure: F)
    where
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, Option<&mut T>, A) -> Result<R, SiltError> + 'gc;
    // F: MethodHandler<'gc, T, A, R> + 'gc;
}

/// Trait for registering fields on UserData types
pub trait UserDataFields<'gc, T: UserData> {
    /// Add a field getter
    fn add_field_method_get<F, R>(&mut self, name: &str, closure: F)
    where
        R: ToLua<'gc>,
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T) -> ToInnerResult<'gc, R> + 'static;

    /// Add a field setter
    fn add_field_method_set<F, V, R>(&mut self, name: &str, closure: F)
    where
        V: FromLua<'gc>,
        R: ToLua<'gc>,
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, V) -> ToInnerResult<'gc, R> + 'static;
}

/// Function pointer for calling a method on a UserData instance
pub type UserDataMethodFn<'gc, T> =
    fn(&VM<'gc>, &Mutation<'gc>, &mut T, Vec<Value<'gc>>) -> InnerResult<'gc>;

/// Function pointer for getting a field from a UserData instance  
pub type UserDataGetterFn<'gc, T> = dyn Fn(&VM<'gc>, &Mutation<'gc>, &T) -> InnerResult<'gc> + 'gc;

/// Function pointer for setting a field on a UserData instance
pub type UserDataSetterFn<'gc, T> =
    dyn Fn(&VM<'gc>, &Mutation<'gc>, &mut T, Value<'gc>) -> InnerResult<'gc> + 'gc;

/// Trait object for type-erased UserData methods
pub trait UserDataMapTraitObj<'gc>: 'gc {
    // fn call_method(
    //     &self,
    //     vm: &VM<'gc>,
    //     mc: &Mutation<'gc>,
    //     ud: &mut UserDataWrapper,
    //     name: &str,
    //     args: Vec<Value<'gc>>,
    // ) -> InnerResult<'gc>;
    fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        index: usize,
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
pub struct UserDataTypedMap<'gc, T: UserData + 'gc> {
    methods: HashMap<String, UserDataMethodClosure<'gc>>,
    // methods2: HashMap<String, dyn MethodHandler<'gc,T,_,_>>,
    // method_cache: Vec<NativeFunctionRc<'gc>>,
    meta_methods: Vec<UserDataMethodClosure<'gc>>,
    getters: HashMap<String, Box<UserDataGetterFn<'gc, T>>>,
    setters: HashMap<String, Box<UserDataSetterFn<'gc, T>>>,
    // type_id: std::any::TypeId,
    _phantom: PhantomData<T>,
}

impl<'gc, T: UserData + 'static> UserDataTypedMap<'gc, T> {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
            // method_cache: Vec::new(),
            meta_methods: Vec::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
            // type_id: std::any::TypeId::of::<T>(),
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

    // Create a NativeFunction that calls a UserData method
    // pub fn create_method_function<'a>(&mut self, mc: &Mutation<'gc>) {
    //     if let Some(&method_fn) = self.methods.get("t") {
    //         let native_fn = move |vm: &mut VM<'gc>,
    //                               mc: &Mutation<'gc>,
    //                               args: Vec<Value<'gc>>|
    //               -> InnerResult<'gc> {
    //             let method_args = args[1..].to_vec();
    //             if let Value::UserData(ud_ref) = args[0] {
    //                 let ud_borrow = ud_ref.borrow();
    //                 if let Ok(mut data_lock) = ud_borrow.data.lock() {
    //                     if let Some(typed_data) = data_lock.downcast_mut::<T>() {
    //                         return method_fn(vm, mc, typed_data, method_args);
    //                     }
    //                 };
    //             }
    //             Err(SiltError::UDBadCast)
    //         };
    //         let r = Rc::new(native_fn);
    //         self.method_cache.push(r.clone());
    //         elf.getters.insert(
    //             st.to_string(),
    //             Box::new(move |_, m, _| {
    //                 Ok(Value::NativeFunction(Gc::new(m, WrappedFn::new(r.clone()))))
    //             }),
    //         );
    //         // Some(Value::NativeFunction(Gc::new(mc, WrappedFn::new(r))))
    //     }
    // }

    // pub fn get_static_item(&self) -> NativeFunctionRef<'gc> {
    //     // let n = Box::new(42);
    //     let b=self.method_cache.last().unwrap();
    //     let rr=Box::leak(b);
    // }

    // Create a NativeFunction that calls a UserData method
    pub fn create_method_function(&mut self) {
        self.methods.drain().for_each(|(st, method_fn)| {
            // let type_id = self.type_id;
            // let method_name = method_name.to_string();

            // let native_fn = move |vm: &mut VM<'gc>,
            //                       mc: &Mutation<'gc>,
            //                       args: &[Value<'gc>]|
            //       -> Value<'gc> {
            //     let method_args = args[1..].to_vec();
            //     // First argument should be the UserData instance
            //     if let Value::UserData(ud_ref) = args[0] {
            //         let ud_borrow = ud_ref.borrow();
            //         if let Ok(mut data_lock) = ud_borrow.data.lock() {
            //             if let Some(typed_data) = data_lock.downcast_mut::<T>() {
            //                 // println!("we're here an dthen{:?}", method_fn);
            //                 for ele in method_args.iter() {
            //                     println!(" args {}", ele);
            //                 }
            //                 // Call the method with remaining arguments
            //                 return method_fn(vm, mc, typed_data, method_args)?;
            //             }
            //         };
            //     }
            //     Err(SiltError::UDBadCast)
            //     // Value::Nil
            // };
            let native_fn = method_fn;
            let raw = NativeFunctionRaw { func: native_fn };
            let r = Rc::new(raw);
            // self.method_cache.push(r.clone());
            self.getters.insert(
                st.to_string(),
                Box::new(move |_, mc, _| {
                    Ok(Value::NativeFunction(Gc::new(
                        mc,
                        WrappedFn::new(r.clone()),
                    )))
                }),
            );
        });
    }
}

impl<'gc, T: UserData + 'static> Default for UserDataTypedMap<'gc, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc, T: UserData + 'static> UserDataMapTraitObj<'gc> for UserDataTypedMap<'gc, T> {
    // fn call_method(
    //     &self,
    //     vm: &VM<'gc>,
    //     mc: &Mutation<'gc>,
    //     ud: &mut UserDataWrapper,
    //     name: &str,
    //     args: Vec<Value<'gc>>,
    // ) -> InnerResult<'gc> {
    //     // TODO use old commit change b0f21fa5
    //     // if let Some(method_fn) = self.methods.get(name) {
    //     //     // Create a mutable VM reference for the method call
    //     //     let vm_mut = unsafe { &mut *(vm as *const VM<'gc> as *mut VM<'gc>) };
    //     //     return method_fn(vm_mut, mc, &args);
    //     // }
    //     Err(SiltError::UDNoMethodRef)
    // }

    fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        index: usize,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        // TODO man this is broken, we can downcast but it wont work with our normal method
        // closure, we need a new thing??
        // if let Some(&method_fn) = self.meta_methods.get(index) {
        //     if let Ok(mut d) = ud.data.lock() {
        //
        //         return match d.downcast_mut() {
        //             Some(typed_ud) => method_fn(vm, mc, typed_ud, args),
        //             None => Err(SiltError::UDBadCast),
        //         };
        //     }
        // }
        Err(SiltError::UDNoMethodRef)
    }

    fn get_field(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &UserDataWrapper,
        name: &str,
    ) -> InnerResult<'gc> {
        // println!("name {}", name);
        if let Some(getter_fn) = self.getters.get(name) {
            // println!("yeah we exist {}", name);
            if let Ok(d) = ud.data.lock() {
                return match d.downcast_ref() {
                    Some(typed_ud) => getter_fn(vm, mc, typed_ud),
                    None => Err(SiltError::UDBadCast),
                };
            }
        }
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
        if let Some(setter_fn) = self.setters.get(name) {
            if let Ok(mut d) = ud.data.lock() {
                return match d.downcast_mut() {
                    Some(typed_ud) => setter_fn(vm, mc, typed_ud, value),
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

    // pub fn call_method(
    //     &self,
    //     vm: &VM<'gc>,
    //     mc: &Mutation<'gc>,
    //     ud: &mut UserDataWrapper,
    //     name: &str,
    //     args: Vec<Value<'gc>>,
    // ) -> InnerResult<'gc> {
    //     self.data.call_method(vm, mc, ud, name, args)
    // }

    pub fn call_meta_method(
        &self,
        vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        ud: &mut UserDataWrapper,
        index: usize,
        args: Vec<Value<'gc>>,
    ) -> InnerResult<'gc> {
        self.data.call_meta_method(vm, mc, ud, index, args)
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

// fn userdata_trap<'a>(vm: &mut VM<'a>, mc: &Mutation<'a>, args: Vec<Value<'a>>) -> Value<'a> {
//     if let Some(Value::UserData(ud)) = args.get(0) {
//         let mut mu = (*ud).borrow_mut(mc);
//         let rud = mu.deref_mut();
//         let type_name = rud.type_name();
//         if let Some(map) = vm.userdata_registry.get_map(type_name) {
//             return map.call_method(vm, mc, rud, method_name, args);
//         }
//     }
//     Value::Nil
// }
//

// trait MyFromLuaMulti<'gc>{
//     type Type<'e>: FromLuaMulti<'gc,'e>;
// }
//
// struct MyHeldValue<'e>(&'e str);
// impl<'gc> MyFromLuaMulti<'gc> for MyHeldValue<'static>{
//     type Type<'e> = MyHeldValue<'e>;
// }

// <V as MyFromLuaMulti<'gc>>::Type<'e>

// type Boo<'gc, T,V,R> = for<'f> fn(
//             &mut VM<'gc>,
//             &Mutation<'gc>,
//             &mut T,
// // <V as FromLuaMulti<'f, 'gc> >::Output
//             V,
//             // <V as FromLuaMulti<'f, 'gc>>::Item,
//         ) -> R where R: ToLua<'gc>;

impl<'gc, T: UserData + 'static> UserDataMethods<'gc, T> for UserDataTypedMap<'gc, T> {
    fn add_meta_method<M, A, R, F>(&mut self, name: M, closure: F)
    where
        M: Into<MetaMethod>,
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        F: MethodHandler<'gc, T, A, R> + 'gc,
    {
        let metamethod: MetaMethod = name.into();
        self.meta_methods.insert(
            metamethod.as_ind(),
            Box::new(move |vm, mc, args| {
                let res = if let Some(ud_val) = args.first() {
                    match ud_val.apply_userdata_mut::<T, _, R>(mc, |ud| {
                        let method_args = &args[1..];
                        closure.call_method(vm, mc, Some(ud), method_args)
                    }) {
                        Ok(rr) => rr,
                        Err(SiltError::UDBadCast) => closure.call_method(vm, mc, None, args)?,
                        Err(e) => return Err(e),
                    }
                } else {
                    //first value not userdata, should fail? or only if REQUIRED? TODO
                    closure.call_method(vm, mc, None, args)?
                };
                R::to_lua(res, vm, mc)
            }),
        );
    }

    fn add_method_mut<A, R, F>(&mut self, name: &str, closure: F)
    where
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        // for<'a> F: Fn(
        //         &mut VM<'gc>,
        //         &Mutation<'gc>,
        //         &mut T,
        //         <A as FromLuaMultiBorrow<'gc>>::Output<'a>,
        //     ) -> Result<R, SiltError>
        //     + 'gc,
        F: MethodHandler<'gc, T, A, R> + 'gc,
    {
        self.methods.insert(
            name.to_string(),
            Box::new(move |vm, mc, args| {
                let res = if let Some(ud_val) = args.first() {
                    match ud_val.apply_userdata_mut::<T, _, R>(mc, |ud| {
                        let method_args = &args[1..];
                        closure.call_method(vm, mc, Some(ud), method_args)
                    }) {
                        Ok(rr) => rr,
                        Err(SiltError::UDBadCast) => closure.call_method(vm, mc, None, args)?,
                        Err(e) => return Err(e),
                    }
                } else {
                    //first value not userdata, should fail? or only if REQUIRED? TODO
                    closure.call_method(vm, mc, None, args)?
                };
                R::to_lua(res, vm, mc)
            }),
        );
        //     if let Some(Value::UserData(ud_ref)) = args.get(0) {
        //         ud_ref.borrow_mut(mc).downcast_mut(apply)
        //         let mut ud_borrow = ud_ref.borrow_mut(mc).downcast_mut(apply);
        //         if let Ok(mut data_lock) = ud_borrow.
        //             if let Some(typed_data) = data_lock.downcast_mut::<T>() {
        //                 // Convert remaining arguments using FromLuaMulti
        //                 let method_args = &args[1..];
        //                 let converted_args = A::from_lua_multi(method_args, vm, mc)?;
        //                 let result = closure(vm, mc, typed_data, converted_args)?;
        //                 return R::to_lua(result, vm, mc);
        //             }
        //         }
        //     }
        //     Err(SiltError::UDBadCast)
        // }),
    }

    fn add_method<A, R, F>(&mut self, name: &str, closure: F)
    where
        A: FromLuaMulti<'gc>,
        R: ToLua<'gc>,
        // F: Fn(
        //         &VM<'gc>,
        //         &Mutation<'gc>,
        //         Option<&T>,
        //         <A as FromLuaMultiBorrow<'gc>>::Output,
        //     ) -> Result<R, SiltError>
        //     + 'gc,
        F: MethodHandler<'gc, T, A, R> + 'gc,
    {
        self.methods.insert(
            name.to_string(),
            Box::new(move |vm, mc, args| {
                let res = if let Some(ud_val) = args.first() {
                    match ud_val.apply_userdata_mut::<T, _, R>(mc, |ud| {
                        let method_args = &args[1..];
                        closure.call_method(vm, mc, Some(ud), method_args)
                    }) {
                        Ok(rr) => rr,
                        Err(SiltError::UDBadCast) => closure.call_method(vm, mc, None, args)?,
                        Err(e) => return Err(e),
                    }
                } else {
                    //first value not userdata, should fail? or only if REQUIRED? TODO
                    closure.call_method(vm, mc, None, args)?
                };
                R::to_lua(res, vm, mc)
            }),
        );
    }
}

impl<'gc, T: UserData + 'static> UserDataFields<'gc, T> for UserDataTypedMap<'gc, T> {
    fn add_field_method_get<F, R>(&mut self, name: &str, closure: F)
    where
        R: ToLua<'gc>,
        F: Fn(&VM<'gc>, &Mutation<'gc>, &T) -> ToInnerResult<'gc, R> + 'gc,
    {
        // println!("add getter {}", name);
        let func: Box<UserDataGetterFn<T>> =
            Box::new(move |vm, mc, ud| R::to_lua(closure(vm, mc, ud), vm, mc));
        self.getters.insert(name.to_string(), func);
    }

    fn add_field_method_set<F, V, R>(&mut self, name: &str, closure: F)
    where
        V: FromLua<'gc>,
        R: ToLua<'gc>,
        F: Fn(&VM<'gc>, &Mutation<'gc>, &mut T, V) -> ToInnerResult<'gc, R> + 'gc,
    {
        // println!("add setter {}", name);
        let func: Box<UserDataSetterFn<T>> = Box::new(move |vm, mc, ud, arg| {
            R::to_lua(closure(vm, mc, ud, V::from_lua(&arg, vm, mc)?), vm, mc)
        });
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
    pub fn register<T: UserData>(&mut self, _mc: &Mutation<'gc>) {
        let type_name = T::type_name();
        // Only register if not already registered
        if !self.maps.contains_key(type_name) {
            let mut typed_map = UserDataTypedMap::<T>::new();

            // Register methods
            T::add_methods(&mut typed_map);

            // Register fields
            T::add_fields(&mut typed_map);
            // println!("TypedMap {}", typed_map.to_string());
            typed_map.create_method_function();
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

    // Get a reference to the wrapped data as Any
    // pub fn as_any(&self) -> &dyn Any {
    //     self.data.as_ref()
    // }

    // Get a mutable reference to the wrapped data as Any
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

    fn to_silt<T>(e: Result<T, impl Error>, res: SiltError) -> Result<T, SiltError> {
        match e {
            Ok(t) => Ok(t),
            Err(_) => Err(res),
        }
    }

    pub fn is_type<T: UserData>(&self) -> bool {
        if let Ok(d) = self.data.lock() {
            d.is::<T>()
        } else {
            false
        }
    }

    // pub fn lock()

    // pub fn downcast_mut2<'a, 'b: 'a, T:'static>(&'b mut self) -> Result<&'a mut T, SiltError> {
    //     let mut i = Self::to_silt(self.data.lock(),SiltError::UDNoMap)?;
    //     (*i).downcast_mut::<T>().ok_or(SiltError::Unknown)
    //     // self.data.lock().unwrap().downcast_mut::<T>().unwrap()
    //     //.downcast_mut::<T>()
    // }
    // pub fn
    pub fn downcast_mut<'a, 'b: 'a, T: UserData, F, R>(
        &'a mut self,
        apply: F,
    ) -> Result<R, SiltError>
    where
        F: FnOnce(&mut T) -> Result<R, SiltError>,
        R: ToLua<'b>,
    {
        let mut i = Self::to_silt(self.data.lock(), SiltError::UDNoMap)?;
        let ud = (*i).downcast_mut::<T>().ok_or(SiltError::UDBadCast)?;
        apply(ud)
    }
pub fn downcast_ref<'a, 'b: 'a, T: UserData, F, R>(
        &'a self,
        apply: F,
    ) -> Result<R, SiltError>
    where
        F: FnOnce(&T) -> Result<R, SiltError>,
        R: ToLua<'b>,
    {
        let i = Self::to_silt(self.data.lock(), SiltError::UDNoMap)?;
        let ud = (*i).downcast_ref::<T>().ok_or(SiltError::UDBadCast)?;
        apply(ud)
    }

    pub fn downcast_get<'a, 'b: 'a, T: UserData, F, R>(&'a self, apply: F) -> Result<R, SiltError>
    where
        F: FnOnce(&T) -> Result<R, SiltError>,
    {
        let i = Self::to_silt(self.data.lock(), SiltError::UDNoMap)?;
        let ud = (*i).downcast_ref::<T>().ok_or(SiltError::UDBadCast)?;
        apply(ud)

        // Ok(Value::Nil)
        //.downcast_mut::<T>()
    }
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
        methods.add_meta_method(
            MetaMethod::ToString,
            |vm, mc, this: Option<&mut TestEnt>, _: ()| {
                let id = if let Some(ud) = this { ud.get_id() } else { 0 };
                Ok(Value::String(format!("[entity {}]", id)))
            },
        );

        methods.add_meta_method("__concat", |vm, mc, this, _: ()| {
            let id = if let Some(ud) = this { ud.get_id() } else { 0 };
            Ok(Value::String(format!("[entity {}]", id)))
        });

        // methods.add_method_mut::<VariadicMaker<Value<'gc>>, _, _>("pos", |_, _, this, arg: VariadicMaker<Value<'gc>>| {
        //     // Example of parsing a table to set position
        //     // if let Some(Value::Table(t)) = arg.first() {
        //     //     let tbl = t.borrow();
        //     //     if let Some(x) = tbl.get("x") {
        //     //         this.x = x.into();
        //     //     }
        //     //     if let Some(y) = tbl.get("y") {
        //     //         this.y = y.into();
        //     //     }
        //     //     if let Some(z) = tbl.get("z") {
        //     //         this.z = z.into();
        //     //     }
        //     // }
        //
        //     // if let Some(Value::Table(t)) = args.first() {
        //     //     let t_ref = (*t).borrow();
        //     //     if let Some(x) = t_ref.get("x") {
        //     //         this.x = x.into();
        //     //     }
        //     //     if let Some(y) = t_ref.get(Value::String("y".to_string())) {
        //     //         this.y = y.into();
        //     //     }
        //     //     if let Some(z) = t_ref.get(Value::String("z".to_string())) {
        //     //         this.z = z.into();
        //     //     }
        //     // }
        //     Ok(())
        // });

        // VariadicMaker<Value<'_>
        // || {
        //     r = convert
        //     out= c(r)
        //     convert_out(out)
        // }
        // Add a method that demonstrates multiple parameters
        // methods.add_method_mut::<'f, Variadic<'f,'gc,Value<'gc>>, _, std::result::Result<(), SiltError>>("set_pos", |vm, mc, this, args: Variadic<Value<'gc>>| {
        // let test: for<'f> fn(
        //     &'f mut VM<'gc>,
        //     &'f Mutation<'gc>,
        //     // &mut T,
        //     < V as FromLuaMulti<'f, 'gc>>::Output<'f> = |vm: &mut VM<'gc>, mc, args| Ok(()));

        methods.add_method_mut("test", |_, _, this, _: ValueRef| {
            // let v = args.deref();
            println!(
                "internal userdata method heehehehe (is self param userdata? {}!)",
                this.is_some()
            );
            Ok(Value::Integer(3))
        });

        methods.add_method_mut("iter", |vm, mc, this, args: Variadic| {
            let ite = args.iter();
            println!("start iterate! we got:");
            ite.for_each(|v| {
                println!("value: {}", v);
            });
            Ok(())
        });

        // let closure = |vm, mc, this, args: (f64,)| Ok(());
        //
        //     let bx = Box::new(move |vm, mc, args: &[Value]| {
        //         if let Some(ud_val) = args.first() {
        //             let r = ud_val.apply_userdata::<TestEnt, _, Value>(mc, |ud| {
        //                 let method_args = &args[1..];
        //                 // let converted = A::from_lua_multi_borrow(method_args, vm, mc)?;
        //                 closure.call_method(vm, mc, ud, method_args)
        //             })?;
        //             // R::to_lua(r, vm, mc)
        //             Ok(Value::Nil)
        //         } else {
        //             //first value not userdata, should fail? or only if REQUIRED? TODO
        //             Err(SiltError::UDBadCast)
        //         }
        //     });
        //
        // let f: TestFn= TestFn{
        //     store: bx};

        // methods.add_method_mut("set_pos", |vm, mc, args: &Value<'gc>| {
        //     Ok(())
        //     // if args.len() >= 3 {
        //     //     if let Value::Number(x) = args[0] {
        //     //         this.x = x;
        //     //     } else if let Value::Integer(x) = args[0] {
        //     //         this.x = x as f64;
        //     //     }
        //     //
        //     //     if let Value::Number(y) = args[1] {
        //     //         this.y = y;
        //     //     } else if let Value::Integer(y) = args[1] {
        //     //         this.y = y as f64;
        //     //     }
        //     //
        //     //     if let Value::Number(z) = args[2] {
        //     //         this.z = z;
        //     //     } else if let Value::Integer(z) = args[2] {
        //     //         this.z = z as f64;
        //     //     }
        //     // }
        //     //
        //     // // Return multiple values as an example
        //     // let mut vals = vm.raw_table();
        //     // vals.push(Value::Number(this.x));
        //     // vals.push(Value::Number(this.y));
        //     // vals.push(Value::Number(this.z));
        //     //
        //     // Ok(vm.wrap_table(mc, vals))
        //     // Ok(Value::Nil)
        // });
    }

    fn add_fields<'gc, F: UserDataFields<'gc, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, _, this| Ok(Value::Number(this.x)));

        // fields.add_field_method_get("t", |vm,mc,this,_|{Ok(Value::Nil)});

        fields.add_field_method_set("x", |_, _, this, val| {
            if let Value::Number(x) = val {
                this.x = x;
            } else if let Value::Integer(x) = val {
                this.x = x as f64;
            }
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
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum MetaMethod {
    #[default]
    None,
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
            MetaMethod::None => write!(f, "!!"),
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

impl<'a> Into<MetaMethod> for &'a str {
    fn into(self) -> MetaMethod {
        match self {
            "__add" => MetaMethod::Add,
            "__sub" => MetaMethod::Sub,
            "__mul" => MetaMethod::Mul,
            "__div" => MetaMethod::Div,
            "__mod" => MetaMethod::Mod,
            "__pow" => MetaMethod::Pow,
            "__unm" => MetaMethod::Unm,
            "__idiv" => MetaMethod::IDiv,
            "__band" => MetaMethod::BAnd,
            "__bor" => MetaMethod::BOr,
            "__bxor" => MetaMethod::BXor,
            "__bnot" => MetaMethod::BNot,
            "__shl" => MetaMethod::Shl,
            "__shr" => MetaMethod::Shr,
            "__concat" => MetaMethod::Concat,
            "__len" => MetaMethod::Len,
            "__eq" => MetaMethod::Eq,
            "__lt" => MetaMethod::Lt,
            "__le" => MetaMethod::Le,
            "__gt" => MetaMethod::Gt,
            "__ge" => MetaMethod::Ge,
            "__index" => MetaMethod::Index,
            "__newindex" => MetaMethod::NewIndex,
            "__call" => MetaMethod::Call,
            "__tostring" => MetaMethod::ToString,
            "__pairs" => MetaMethod::Pairs,
            "__ipairs" => MetaMethod::IPairs,
            _ => panic!("Unknown metamethod: {}", self),
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
            MetaMethod::None => "__",
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
    fn as_ind(&self) -> usize {
        match self {
            MetaMethod::None => 0,
            MetaMethod::Add => 1,
            MetaMethod::Sub => 2,
            MetaMethod::Mul => 3,
            MetaMethod::Div => 4,
            MetaMethod::Mod => 5,
            MetaMethod::Pow => 6,
            MetaMethod::Unm => 7,
            MetaMethod::IDiv => 8,
            MetaMethod::BAnd => 9,
            MetaMethod::BOr => 10,
            MetaMethod::BXor => 11,
            MetaMethod::BNot => 12,
            MetaMethod::Shl => 13,
            MetaMethod::Shr => 14,
            MetaMethod::Concat => 15,
            MetaMethod::Len => 16,
            MetaMethod::Eq => 17,
            MetaMethod::Lt => 18,
            MetaMethod::Le => 19,
            MetaMethod::Gt => 20,
            MetaMethod::Ge => 21,
            MetaMethod::Index => 22,
            MetaMethod::NewIndex => 23,
            MetaMethod::Call => 24,
            MetaMethod::ToString => 25,
            MetaMethod::Pairs => 26,
            MetaMethod::IPairs => 27,
        }
    }
}

/// Helper functions for VM integration
pub mod vm_integration {
    use gc_arena::lock::RefLock;

    use super::*;
    use crate::lua::{UDVec, VM};

    /// Create a new UserData value
    pub fn create_userdata_raw<'gc, T: UserData>(
        reg: &mut UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        data: T,
        userdata_stack: &mut Option<UDVec>,
    ) -> UserDataWrapper {
        // Register the type if it hasn't been registered yet
        let type_name = T::type_name();
        if !reg.maps.contains_key(type_name) {
            // println!(" register userdata");
            reg.register::<T>(mc);
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
        wrapper

        // Create the GC-managed wrapper
    }

    /// Create a new UserData value
    pub fn create_userdata<'gc, T: UserData>(
        reg: &mut UserDataRegistry<'gc>,
        mc: &Mutation<'gc>,
        data: T,
        userdata_stack: &mut Option<UDVec>,
    ) -> Value<'gc> {
        let ud = create_userdata_raw(reg, mc, data, userdata_stack);

        let ud_gc = Gc::new(mc, RefLock::new(ud));

        // Set the stack index in the GC-managed wrapper
        // ud_gc.borrow_mut(mc).set_stack_index(index);

        // Return as a Value
        Value::UserData(ud_gc)
    }

    /// Call a method on a UserData value
    // pub fn call_method<'gc>(
    //     vm: &VM<'gc>,
    //     reg: &UserDataRegistry<'gc>,
    //     mc: &Mutation<'gc>,
    //     userdata: &mut UserDataWrapper,
    //     method_name: &str,
    //     args: Vec<Value<'gc>>,
    // ) -> Result<Value<'gc>, SiltError> {
    //     let type_name = userdata.type_name();
    //
    //     // Look up the method in the registry
    //     if let Some(map) = reg.get_map(type_name) {
    //         return map.call_method(vm, mc, userdata, method_name, args);
    //     }
    //
    //     Err(SiltError::UDNoMap)
    // }

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
        let meta_key = meta_method.as_ind();

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
