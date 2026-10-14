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
    /// `proto.chunk.code`, borrowed for `'gc` so the dispatch loop indexes one
    /// slice instead of walking Gc -> FunctionObject -> Chunk -> Vec each step.
    code: &'gc [OpCode],
    /// Stack height when this frame was entered; RETURN truncates back to it.
    pub stack_snapshot: usize,
    /// Index of this frame's slot 0 in the VM stack (the function value; locals
    /// follow). For variadic calls it sits above the overflow args.
    pub base: usize,
    /// Index of the current instruction in `code`.
    pub ip: usize,
    /// Call frame is made aware of how many variables need a return and will pop that amount until nil on return
    pub multi_return: u8,
    // how many values we're calling the function with, potential for varargs
    pub call_arity: u8,
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
        let code = Gc::as_ref(proto).chunk.code.as_slice();
        Self {
            function,
            proto,
            code,
            ip: 0,
            base: 0,
            stack_snapshot,
            call_arity,
            multi_return,
        }
    }

    #[inline]
    pub fn current_instruction(&self) -> &crate::code::OpCode {
        &self.code[self.ip]
    }

    /** shift ip by 1 instruction */
    #[inline]
    pub fn iterate(&mut self) {
        self.ip += 1;
    }

    /** does not shift ip, only returns the `n` instructions after the current one */
    pub fn get_next_n_codes(&self, n: usize) -> &[OpCode] {
        &self.code[self.ip + 1..self.ip + 1 + n]
    }

    /** move ip N instructions over */
    pub fn shift(&mut self, n: usize) {
        self.ip += n;
    }

    /// VM stack index of local `index`.
    #[inline]
    pub fn slot(&self, index: u8) -> usize {
        self.base + index as usize
    }

    #[inline]
    pub fn set_val(&self, stack: &mut [Value<'frame>], index: u8, value: Value<'frame>) {
        stack[self.slot(index)] = value;
    }

    #[inline]
    pub fn get_val<'s>(&self, stack: &'s [Value<'frame>], index: u8) -> &'s Value<'frame> {
        &stack[self.slot(index)]
    }

    /// Read the variadic overflow values for this frame. In a variadic call the
    /// extra arguments are left on the stack *below* the frame base (the fixed
    /// params and function are copied above them at call time), so the `nextra`
    /// values sit at `stack[base - nextra .. base]`.
    pub fn get_varargs<'s>(&self, stack: &'s [Value<'frame>], nextra: u8) -> &'s [Value<'frame>] {
        &stack[self.base - nextra as usize..self.base]
    }

    #[inline]
    pub fn get_val_mut<'s>(
        &self,
        stack: &'s mut [Value<'frame>],
        index: u8,
    ) -> &'s mut Value<'frame> {
        &mut stack[self.slot(index)]
    }

    #[cfg(feature = "dev-out")]
    pub fn print_local_stack(&self, stack: &[Value<'frame>]) {
        let end = (self.base + 10).min(stack.len());
        print!("local stack: {:?}", &stack[self.base..end]);
        println!("(top: {})", stack[self.base]);
    }

    #[inline]
    pub fn forward(&mut self, offset: u16) {
        self.ip += offset as usize;
    }

    #[inline]
    pub fn rewind(&mut self, offset: u16) {
        self.ip -= offset as usize;
    }

    /// Source span of the instruction `ip` points at. `ip` indexes the
    /// effective prototype's code (see `proto`), which is parallel to the
    /// `locations` table.
    pub fn current_span(&self) -> ((usize, usize), (usize, usize)) {
        self.proto.chunk.get_span(self.ip)
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

        vm.push(Value::Closure(Gc::new(ep.mc, closure)));
        // }
        // else {
        //     return Err(SiltError::VmRuntimeError);
        // }
        frame.shift(func.upvalue_count as usize);
        Ok(())
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
    pub fn get_arity(&self) -> u8 {
        self.function.arity
    }

    #[allow(dead_code)]
    pub fn print_upvalues(&self) {
        self.upvalues.iter().enumerate().for_each(|(i, f)| {
            println!("fn-up {}:{}", i, f.borrow());
        });
    }
}

/// A captured local. While the local's frame is live the upvalue is *open* and
/// aliases its VM stack slot; when the frame returns (or the block ends) it is
/// *closed* and the value moves into `closed`, so surviving closures keep it.
#[derive(Collect)]
#[collect(no_drop)]
pub struct UpValue<'lua> {
    pub index: u8,
    /** the value once closed */
    closed: Value<'lua>,
    /** the VM stack slot while open; `None` once closed */
    slot: Option<usize>,
}
impl<'lua> UpValue<'lua> {
    pub fn new(index: u8, slot: usize) -> Self {
        Self {
            index,
            closed: Value::Nil,
            slot: Some(slot),
        }
    }

    /// The stack slot this upvalue aliases, or `None` once closed.
    #[inline]
    pub fn slot(&self) -> Option<usize> {
        self.slot
    }

    pub fn set_value(&mut self, stack: &mut [Value<'lua>], value: Value<'lua>) {
        match self.slot {
            Some(s) => stack[s] = value,
            None => self.closed = value,
        }
    }

    pub fn close_around(&mut self, value: Value<'lua>) {
        self.closed = value;
        self.slot = None;
    }

    /// Move the value out of its stack slot (leaving nil) and close.
    pub fn close(&mut self, stack: &mut [Value<'lua>]) {
        if let Some(s) = self.slot.take() {
            self.closed = std::mem::take(&mut stack[s]);
        }
        #[cfg(feature = "dev-out")]
        println!("closed: {}", self.closed);
    }

    /// Close, copying the value and leaving the stack slot intact — for a frame whose
    /// slots are still about to be read (the root RETURN pops its result afterwards).
    pub fn close_keep(&mut self, stack: &[Value<'lua>]) {
        if let Some(s) = self.slot.take() {
            self.closed = stack[s].clone();
        }
    }

    pub fn copy_value(&self, stack: &[Value<'lua>]) -> Value<'lua> {
        match self.slot {
            Some(s) => stack[s].clone(),
            None => self.closed.clone(),
        }
    }
}

impl Display for UpValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.slot {
            Some(s) => write!(f, "⬆️open@slot{}#{}", s, self.index),
            None => write!(f, "⬆️{}#{}", self.closed, self.index),
        }
    }
}
