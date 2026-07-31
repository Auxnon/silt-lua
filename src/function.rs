use std::{fmt::Display, rc::Rc};

use colored::Colorize;
use gc_arena::{lock::RefLock, Collect, Gc, Mutation};

use crate::{
    chunk::Chunk,
    code::OpCode,
    error::SiltError,
    lua::{Ephemeral, VM},
    value::{FromLuaMulti, ToLuaMulti, Value},
};

/////////////
///
pub struct CallFrame<'gc> {
    pub function: Gc<'gc, Closure<'gc>>, // pointer
    /// Effective code prototype for this frame: the closure's prototype, or its
    /// hot-swap redirect target if one is set. All code/constant/arity reads go
    /// through this (see `VM::get_chunk`) so a hot-swapped body executes while the
    /// closure keeps its own upvalues.
    pub proto: Gc<'gc, FunctionObject<'gc>>,
    // ip: *const OpCode
    // pub base: usize,
    // pointer points into VM values stack
    pub stack_snapshot: usize,
    pub local_stack: *mut Value<'gc>,
    pub ip: *const OpCode,
    // pub need: u8,
    /// Call frame is made aware of how many variables need a return and will pop that amount until nil on return
    pub multi_return: u8,
    // how many values we're calling the function with, potential for varargs
    pub call_arity: u8,
    // pub mark: usize
}

impl<'frame> CallFrame<'frame> {
    pub fn new<'a>(
        function: Gc<'frame, Closure<'frame>>,
        stack_snapshot: usize,
        call_arity: u8,
        multi_return: u8,
    ) -> Self {
        // Resolve the effective prototype. With `hot-swap`, follow the redirect cell
        // if a hot-swap set one (stage 2); all live closures share one prototype, so a
        // redirect set on it reaches every instance while each keeps its own upvalues.
        // Without the feature there is no redirect — `proto` is just the closure's
        // prototype, costing nothing beyond a pointer copy (and it lets `get_chunk`
        // skip one indirection on every constant access).
        #[cfg(feature = "hot-swap")]
        let proto = match *function.function.swap.borrow() {
            Some(redirect) => redirect,
            None => function.function,
        };
        #[cfg(not(feature = "hot-swap"))]
        let proto = function.function;
        let ip = proto.chunk.code.as_ptr();
        Self {
            function,
            proto,
            ip,
            local_stack: std::ptr::null_mut(),
            stack_snapshot,
            call_arity,
            multi_return,
        }
    }

    #[inline]
    pub fn current_instruction(&self) -> &crate::code::OpCode {
        // &self.function.chunk.code[self.ip]
        unsafe { &*self.ip }
    }

    /** shift ip by 1 instruction */
    #[inline]
    pub fn iterate(&mut self) {
        // self.ip += 1;
        self.ip = unsafe { self.ip.add(1) };
    }

    /** DANGER: does not shift ip, only returns instruction set in range past ip */
    pub fn get_next_n_codes(&self, n: usize) -> &[OpCode] {
        // &self.function.chunk.code[self.ip..self.ip + n]
        const SIZE: usize = std::mem::size_of::<OpCode>();
        unsafe { std::slice::from_raw_parts(self.ip.add(1), n * SIZE) }
    }

    /** move ip N instructions over */
    pub fn shift(&mut self, n: usize) {
        // self.ip += n;
        self.ip = unsafe { self.ip.add(n) };
    }

    #[inline]
    pub fn set_val(&mut self, index: u8, value: Value<'frame>) {
        // self.stack[index as usize] = value;
        unsafe { *self.local_stack.add(index as usize) = value };
    }

    #[inline]
    pub fn get_val(&self, index: u8) -> &Value<'frame> {
        // &self.stack[index as usize]
        // println!("get_val: {}", index);
        // println!("top: {}", unsafe { &*self.local_stack });
        unsafe { &*self.local_stack.add(index as usize) }
    }

    /// get a slice of size count from local stack offset by index.
    /// Exclusive to VarArg
    pub fn get_vals(&self, index: u8, count: u8) -> &[Value<'frame>] {
        // &self.stack[index as usize]
        unsafe {
            let i = self.local_stack.add((index) as usize);
            std::slice::from_raw_parts(i, count as usize)
        }
    }

    /// Read the variadic overflow values for this frame. In a variadic call the
    /// extra arguments are left on the stack *below* the frame base (the fixed
    /// params and function are copied above them at call time), so the `nextra`
    /// values sit at `local_stack[-nextra .. 0]`.
    pub fn get_varargs(&self, nextra: u8) -> &[Value<'frame>] {
        unsafe {
            let start = self.local_stack.sub(nextra as usize);
            std::slice::from_raw_parts(start, nextra as usize)
        }
    }

    // get_vararg

    #[inline]
    pub fn get_val_mut(&mut self, index: u8) -> &mut Value<'frame> {
        unsafe { &mut *self.local_stack.add(index as usize) }
    }

    #[cfg(feature = "dev-out")]
    pub fn print_local_stack(&self) {
        print!("local stack: {:?}", unsafe {
            std::slice::from_raw_parts(self.local_stack, 10)
        });
        println!("(top: {})", unsafe { &*self.local_stack });
    }

    // pub fn push(&mut self, value: Value) {
    //     // TODO can we push to the stack by pointer? Or should we just push on a Vec?
    //     // *self.stack_top= value;

    //     // unsafe { *self.stack_top = value };
    //     // self.stack.push(value);
    //     // self.stack_top = self.stack.as_ptr().add(self.stack.len());

    //     // unsafe { *self.stack_top = value };
    //     // self.stack_top = unsafe { self.stack_top.add(1) };
    //     self.stack.push(value);
    // }

    // pub fn push(&mut self, value: Value) {
    //     println!("pushing: {}", value);
    //     // self.stack.push(value);
    //     self.stack = unsafe { self.stack.add(1) };
    //     unsafe { *self.stack = value };
    // }

    /** pop and return top of stack */
    // pub fn pop(&mut self) -> Value {
    //     // self.stack.pop().unwrap()
    //     // let d = self.stack.wrapping_add(1);
    //     // take value
    //     let v = unsafe { std::mem::replace(&mut *self.stack, Value::Nil) };
    //     self.stack = unsafe { self.stack.sub(1) };
    //     v

    //     // let o = unsafe { &*self.stack };
    //     // o
    // }

    /** pop N number of values from stack */
    // pub fn popn(&mut self, n: u8) {
    //     // self.stack.truncate(self.stack.len() - n as usize);
    //     self.stack = unsafe { self.stack.sub(n as usize) };
    // }

    /** take and replace with a Nil */
    #[allow(dead_code)]
    pub fn take<'a>(&'frame mut self) -> &'a Value<'frame> {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        let v = unsafe { &*self.local_stack };
        unsafe { *self.local_stack = Value::Nil };
        v
    }

    // TODO validate safety of this, compiler has to be solid af!
    #[inline]
    pub fn forward(&mut self, offset: u16) {
        // self.ip += offset as usize;
        self.ip = unsafe { self.ip.add(offset as usize) };
    }

    #[inline]
    pub fn rewind(&mut self, offset: u16) {
        // self.ip -= offset as usize;
        self.ip = unsafe { self.ip.sub(offset as usize) };
        // println!("rewind: {}", unsafe { &*self.ip });
    }
    pub fn get_loc_by_count(&self, count: usize) -> (usize, usize) {
        let i = count - self.stack_snapshot;
        self.function.function.chunk.get_loc(i)
    }
}
#[derive(Default, Collect)]
#[collect(no_drop)]
pub struct FunctionObject<'chnk> {
    pub is_script: bool,
    pub name: Option<String>,
    pub chunk: Chunk<'chnk>,
    pub upvalue_count: u8,
    pub need: u8,
    pub arity: u8,
    pub is_variadic: bool,
    pub varidic_index: u8,
    /// Source line of the `function` keyword (1-indexed).  Used by the hotswap
    /// engine to map compiled function objects back to their source positions
    /// without requiring a secondary parse pass.  Defaults to 0 for the root
    /// script object and for functions compiled before this field was added.
    pub start_line: usize,
    /// Source line of the matching `end` keyword (1-indexed).  Captured from the
    /// lexer token during `block()` compilation, giving the exact closing line of
    /// the function body for hotswap range detection.  Defaults to 0.
    pub end_line: usize,
    /// Index of the source this function object was compiled from, assigned by the
    /// compiler and shared by the root chunk and every nested function compiled in
    /// the same pass. A runtime error stamps this onto the `ErrorOut` so the caller
    /// can look up the originating source. `usize::MAX` = untracked.
    pub source_index: usize,
    /// Hot-swap redirect cell (stage 2). `None` normally. When a function body is
    /// hot-swapped, this *shared* prototype's cell is set to the newly-compiled
    /// prototype; because every live closure of a definition points at the same
    /// prototype, setting this redirects them all to the new code on their next
    /// call while each keeps its own captured upvalues (instance state). The
    /// redirect target itself always has `swap == None`, so resolution is a single
    /// hop from the original prototype.
    ///
    /// Gated behind `hot-swap`: builds without the feature carry neither this field
    /// nor the per-call redirect check, so the normal execution path is unchanged.
    #[cfg(feature = "hot-swap")]
    pub swap: RefLock<Option<Gc<'chnk, FunctionObject<'chnk>>>>,
}

impl<'chnk> FunctionObject<'chnk> {
    pub fn new(name: Option<String>, is_script: bool) -> Self {
        Self {
            name,
            is_script,
            chunk: Chunk::new(),
            upvalue_count: 0,
            need: 1,
            arity: 0,
            is_variadic: false,
            varidic_index: 0,
            start_line: 0,
            end_line: 0,
            source_index: crate::error::SOURCE_INDEX_UNKNOWN,
            #[cfg(feature = "hot-swap")]
            swap: RefLock::new(None),
        }
    }

    pub fn set_chunk(&mut self, chunk: Chunk<'chnk>) {
        self.chunk = chunk;
    }

    pub(crate) fn push_closure<'a>(
        func: Gc<'a, FunctionObject<'a>>,
        vm: &mut VM<'a>,
        frame: &mut CallFrame<'a>,
        ep: &mut Ephemeral<'_, 'a>,
    ) -> Result<(), SiltError> {
        // f.upvalue_count
        let mut closure = Closure::new(func, Vec::with_capacity(func.upvalue_count as usize));
        // if func.upvalue_count > 0 {
        let next_instruction = frame.get_next_n_codes(func.upvalue_count as usize);
        for i in 0..func.upvalue_count {
            // devout!(" | {}", next_instruction[i as usize]);
            if let OpCode::REGISTER_UPVALUE { index, neighboring } = next_instruction[i as usize] {
                closure.upvalues.push(if neighboring {
                    // insert at i
                    vm.capture_upvalue(ep, index, frame)
                    // closure.upvalues.insert(
                    //     i as usize,
                    //     frame.function.upvalues[index as usize].clone(),
                    // );
                    // *slots.add(index) ?
                } else {
                    frame.function.upvalues[index as usize].clone()
                });
            } else {
                println!(
                    "next instruction is not CLOSE_UPVALUE {}",
                    next_instruction[i as usize]
                );
                unreachable!()
            }
        }

        vm.push(ep, Value::Closure(Gc::new(ep.mc, closure)));
        // }
        // else {
        //     return Err(SiltError::VmRuntimeError);
        // }
        frame.shift(func.upvalue_count as usize);
        Ok(())
    }

    #[allow(dead_code)]
    #[allow(unused_assignments)]
    pub(crate) fn call_closure<'gc, 'a: 'gc>(
        clos: &'a Gc<Closure<'gc>>,
        frames: &'a mut Vec<CallFrame<'gc>>,
        stack_count: usize,
        arity: usize,
        mut frame: &'a mut CallFrame<'gc>,
        ep: &mut Ephemeral<'_, 'a>,
    ) {
        let frame_top = unsafe { ep.ip.sub(arity + 1) };
        let new_frame = CallFrame::new(clos.clone(), stack_count - arity - 1, arity as u8, 0);
        frames.push(new_frame);
        frame = frames.last_mut().unwrap();
        frame.local_stack = frame_top;
    }

    pub fn print(&self) {
        self.chunk.print_chunk(&self.name);
    }
}

impl Display for FunctionObject<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_script {
            write!(
                f,
                "module={}",
                self.name.as_ref().unwrap_or(&"root".to_string())
            )
        } else {
            write!(
                f,
                "fn {}()",
                self.name.as_ref().unwrap_or(&"anonymous".to_string())
            )
        }
    }
}

// pub type NativeFunction<'lua> = &'static fn(&'lua mut Lua, Vec<Value>) -> Value<'lua>; // TODO should be Result<Value,SiltError> for runtime errors
// pub struct NativeObject {
//     name: &'static str,
//     pub function: NativeFunction<'static>,
// }

// pub type NativeFunctionRaw<'a, T: FromLuaMulti<'a>> =
//     dyn Fn(&mut VM<'a>, &Mutation<'a>, impl FromLuaMulti<'a>) -> InnerResult<'a> + 'a;

// pub type NativeFunctionRaw<'a> = NativeFunctionS<'a>;

#[allow(dead_code)]
pub type NativeFunctionRef<'a> = &'a NativeFunctionRaw<'a>;
pub type NativeFunctionRc<'a> = Rc<NativeFunctionRaw<'a>>;
// pub trait NativeFunction<'a> =  Fn(&mut VM<'a>, &Mutation<'a>, Vec<Value<'a>>) -> Value<'a>;

/// What a native function hands back. Most functions return a single value;
/// multi-return functions (`next`, `pairs`, `ipairs`, `select`, `pcall`, …)
/// return several, which the CALL handler spreads onto the stack.
pub enum NativeReturn<'gc> {
    Single(Value<'gc>),
    Multi(Vec<Value<'gc>>),
}

pub type NativeResult<'gc> = Result<NativeReturn<'gc>, SiltError>;

pub struct NativeFunctionRaw<'a> {
    pub func: Box<dyn Fn(&mut VM<'a>, &Mutation<'a>, &[Value<'a>]) -> NativeResult<'a> + 'a>,
}

impl<'gc> NativeFunctionRaw<'gc> {
    pub fn new<A, F, R>(f: F) -> Self
    where
        A: FromLuaMulti<'gc>,
        R: ToLuaMulti<'gc>,
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, A) -> Result<R, SiltError> + 'gc,
    {
        Self {
            func: Box::new(move |vm, mc, raw_args| {
                let args = A::from_lua_multi(raw_args, vm, mc)?;
                // The `?` handles the error; `R` is the SUCCESS type, so a tuple return
                // spreads (Multi), a scalar/collection is one value (Single/table), and
                // `to_native_return` keeps the scalar path allocation-free.
                f(vm, mc, args)?.to_native_return(vm, mc)
            }),
        }
    }

    /// Register a native function that returns multiple values. The closure
    /// takes the raw argument slice and yields a `Vec<Value>`.
    pub fn new_multi<F>(f: F) -> Self
    where
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, &[Value<'gc>]) -> Result<Vec<Value<'gc>>, SiltError>
            + 'gc,
    {
        Self {
            func: Box::new(move |vm, mc, raw_args| Ok(NativeReturn::Multi(f(vm, mc, raw_args)?))),
        }
    }

    // /// Helper method that infers types from closure signature
    // pub fn from_closure<F, T, R>(f: F) -> Self
    // where
    //     R: ToLua<'a>,
    //     T: FromLuaMulti<'a>,
    //     F: for <'f> Fn(&mut VM<'a>, &Mutation<'a>, T::Args<'f>) -> ToInnerResult<'a, R> + 'a,
    // {
    //     Self::new::<T, F, R>(f)
    // }

    pub fn call(
        &self,
        vm: &mut VM<'gc>,
        mutation: &Mutation<'gc>,
        args: &[Value<'gc>],
    ) -> NativeResult<'gc> {
        (self.func)(vm, mutation, args)
    }
}

// native        (vm, mc, val[])-> res
// meth          (vm, mc, &ud, val[])-> res
// meth_meta     (vm, mc, &ud, val[])-> res
// meth_mut      (vm, mc, &mut ud, val[])-> res

pub struct WrappedFn<'gc> {
    // pub f: Box<dyn Fn(&mut VM<'gc>, &Mutation<'gc>, Vec<Value<'gc>>) -> InnerResult<'gc>>, // used exclusively by userdata, a bit of a hack
    pub f: NativeFunctionRc<'gc>,
    // pub meta: u8
}

impl<'gc> WrappedFn<'gc> {
    pub fn new(
        // callback: Rc<dyn Fn(&mut VM<'gc>, &Mutation<'gc>, T) -> InnerResult<'gc> + 'gc>,
        callback: NativeFunctionRc<'gc>,
    ) -> Self {
        Self { f: callback }
    }

    pub fn call(
        &self,
        vm: &mut VM<'gc>,
        mc: &Mutation<'gc>,
        args: &[Value<'gc>],
    ) -> NativeResult<'gc> {
        (self.f.func)(vm, mc, args)
    }
}

// pub struct WrappedFn<N>
//
//         where
//             N: for<'a> Fn(&mut VM<'a>, &Mutation<'a>, Vec<Value<'a>>)-> Value<'a>,
//
// {
//     pub f: N}

// impl<'gc> Deref for WrappedFn<'gc> {
//     type Target = dyn Fn(&mut VM<'gc>, &Mutation<'gc>, Vec<Value<'gc>>) -> Value<'gc>;
//         fn deref(&self) -> &Self::Target {
//         &self.f
//     }
// }

// impl WrappedFn{
//     pub fn call(&self, vm: &mut VM<'lua>, m: &Mutation<'lua>, vals: Vec<Value<'lua>>)-> Value{
//         self.f(vm,m,vals)
//     }
// }

unsafe impl<'gc> Collect for WrappedFn<'gc> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }
}

// impl NativeObject {
//     pub fn new(name: &'static str, function: NativeFunction) -> Self {
//         Self { name, function }
//     }
// }

#[derive(Collect)]
#[collect(no_drop)]
pub struct Closure<'lua> {
    pub function: Gc<'lua, FunctionObject<'lua>>,
    pub upvalues: Vec<Gc<'lua, RefLock<UpValue<'lua>>>>,
}

impl<'chnk> Closure<'chnk> {
    pub fn new(
        function: Gc<'chnk, FunctionObject<'chnk>>,
        upvalues: Vec<Gc<'chnk, RefLock<UpValue<'chnk>>>>,
    ) -> Self {
        Self { function, upvalues }
    }

    pub fn is_variadic(&self) -> bool {
        self.function.is_variadic
    }
    pub fn get_variadic(&self) -> u8 {
        self.function.varidic_index
    }
    pub fn get_arity(&self)->u8{
        self.function.arity
}

    pub fn print_upvalues(&self) {
        self.upvalues.iter().enumerate().for_each(|(i, f)| {
            println!("fn-up {}:{}", i, f.borrow());
        });
    }
}

pub struct UpValue<'lua> {
    // is_open: bool,
    // obj?
    pub index: u8,
    /** the value */
    closed: Value<'lua>,
    // pub location: NonNull<Value<'lua>>,
    /** the pointer to the closed value */
    pub location: *mut Value<'lua>,
    // pub value: *mut Value, // TODO oshould be a RC mutex of the value ideally
}
impl<'lua> UpValue<'lua> {
    pub fn new(index: u8, location: *mut Value<'lua>) -> Self {
        Self {
            index,
            closed: Value::Nil,
            location,
        }
    }

    pub fn set_value(&mut self, value: Value<'lua>) {
        unsafe { *self.location = value }
    }

    pub fn close_around(&mut self, value: Value<'lua>) {
        self.closed = value;
        self.location = &mut self.closed as *mut Value;
    }

    pub fn close(&mut self) {
        #[cfg(feature = "dev-out")]
        println!("closing: {}", unsafe { &*self.location });
        self.closed = unsafe { self.location.replace(Value::Nil) };
        #[cfg(feature = "dev-out")]
        println!("closed: {}", self.closed);
        self.location = &mut self.closed as *mut Value;
    }

    pub fn copy_value(&self) -> Value<'lua> {
        #[cfg(feature = "dev-out")]
        println!("copying: {}", unsafe { &*self.location });
        unsafe { (*self.location).clone() }
    }

    pub fn get_location(&self) -> *mut Value<'lua> {
        #[cfg(feature = "dev-out")]
        println!("getting location: {}", unsafe { &*self.location });
        self.location
    }

    // pub fn get(&self) -> &Value {
    //     unsafe { &*self.value }
    // }
    // pub fn set(&mut self, value: Value) {
    //     unsafe { *self.value = value };
    // }
}

unsafe impl<'lua> Collect for UpValue<'lua> {
    fn trace(&self, cc: &gc_arena::Collection) {
        // location is just a helper, it's a pointer to closed, is that stupid? Probably.
        self.closed.trace(cc);
    }
}

impl Display for UpValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "⬆️{}x{}@{}",
            unsafe { &*self.location },
            self.closed,
            self.index
        )
    }
}
