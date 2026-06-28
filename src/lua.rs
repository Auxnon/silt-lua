use std::{borrow::BorrowMut, cell::RefCell, mem::take, ops::DerefMut, rc::Rc};

use colored::Colorize;
use gc_arena::{lock::RefLock, Arena, Collect, Gc, Mutation, Rootable};

use crate::{
    code::OpCode,
    compiler::Compiler,
    error::{ErrorOut, ErrorTuple, SiltError, ValueTypes},
    function::{
        CallFrame, Closure, FunctionObject, NativeFunctionRaw, NativeReturn, UpValue, WrappedFn,
    },
    prelude::UserData,
    table::{ExTable, Table},
    userdata::{InnerResult, MetaMethod, UserDataRegistry, UserDataWrapper, WeakWrapper},
    value::{ExVal, FromLuaMulti, ToLua, ToLuaMulti, Value},
};

/** Convert Integer to Float, lossy for now */
macro_rules! int2f {
    ($left:ident) => {
        $left as f64
    };
}

macro_rules! devout {
    ($($arg:tt)*) => {
        #[cfg(feature = "dev-out")]
        println!($($arg)*);
    }

}

macro_rules! bubble {
    ($arg:expr) => {
        match $arg {
            Ok(o) => o,
            Err(e) => break Err(e),
        }
    };
}

macro_rules! str_op_str{
    ($left:ident $op:tt $right:ident $enu:ident )=>{
        ({
            let mut out=Value::Nil;
            if let Ok(n1) = $left.parse::<i64>() {
                if let Ok(n2) = $right.parse::<i64>() {
                    out=Value::Integer(n1 $op n2)
                }else if let Ok(n2) = $right.parse::<f64>() {
                    out=Value::Number(int2f!(n1) $op n2)
                }
            }else if let Ok(n1) = $left.parse::<f64>() {
                if let Ok(n2) = $right.parse::<f64>() {
                    out=(Value::Number(n1 $op n2));
                }
            }

            if out==Value::Nil{
                break Err(SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                MetaMethod::$enu,
                ValueTypes::String,
                ));
            }
            out
        })
    }
}

macro_rules! str_op_int{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        {
            if let Ok(n1) = $left.parse::<i64>() {
                    Value::Integer(n1 $op $right)
            }
            else if let Ok(n1) = $left.parse::<f64>() {
                    Value::Number(n1 $op int2f!($right))
            }else{
            break Err(SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                MetaMethod::$enu,
                ValueTypes::Integer,
            ));
            }
        }
    }
}

macro_rules! int_op_str{
    ($left:ident $op:tt $right:ident  $enu:ident)=>{
        {
            if let Ok(n1) = $right.parse::<i64>() {
                    Value::Integer($left $op n1)

            }else if let Ok(n1) = $right.parse::<f64>() {
                    Value::Number(int2f!($left) $op n1)
            }else {
            break Err(SiltError::ExpOpValueWithValue(
                ValueTypes::Integer,
                MetaMethod::$enu,
                ValueTypes::String,
            ));
            }
        }
    }
}

// macro_rules! op_error {
//     ($left:ident $op:ident $right:ident ) => {{
//         return Err(SiltError::ExpOpValueWithValue(
//             $left,
//             MetaMethod::$op,
//             $right,
//         ));
//     }};
// }

macro_rules! str_op_num{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        if let Ok(n1) = $left.parse::<f64>() {
            Value::Number(n1 $op $right)
        }else {
            break Err(SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                MetaMethod::$enu,
                ValueTypes::String,
            ))
        }
    }
}

macro_rules! num_op_str{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        if let Ok(n1) = $right.parse::<f64>() {
            Value::Number($left $op n1)
        }else{
            break Err(SiltError::ExpOpValueWithValue(
                ValueTypes::Number,
                MetaMethod::$enu,
                ValueTypes::String,
            ))
        }
    }
}

/// Coerce a value to an integer for bitwise ops, per Lua: integers pass through,
/// floats with an exact integer value convert, everything else has "no integer
/// representation" and is rejected by the caller.
fn bit_int(v: &Value<'_>) -> Option<i64> {
    match v {
        Value::Integer(i) => Some(*i),
        Value::Number(f) => {
            if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 {
                Some(*f as i64)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Lua logical left shift on 64 bits: shifts >= 64 give 0, negative counts shift
/// the other direction.
fn lua_shl(a: i64, b: i64) -> i64 {
    if b <= -64 || b >= 64 {
        0
    } else if b >= 0 {
        ((a as u64).wrapping_shl(b as u32)) as i64
    } else {
        ((a as u64).wrapping_shr((-b) as u32)) as i64
    }
}

/// Lua logical right shift (mirror of `lua_shl`).
fn lua_shr(a: i64, b: i64) -> i64 {
    if b <= -64 || b >= 64 {
        0
    } else if b >= 0 {
        ((a as u64).wrapping_shr(b as u32)) as i64
    } else {
        ((a as u64).wrapping_shl((-b) as u32)) as i64
    }
}

macro_rules! binary_op_push {
    ($src:ident, $ep:ident, $frame:ident, $frames:ident, $frame_count:ident, $op:tt, $opp:tt) => {{
        #[cfg(feature = "dev-out")]
        $src.body.chunk.print_constants();
        // TODO test speed of this vs 1 pop and a mutate
        let r = $src.pop($ep);
        let l = $src.pop($ep);
        let res = binary_op!($src, $ep, $frame, $frames, $frame_count, l, $op, r, $opp);

        $src.push($ep, res);
    }};
}

macro_rules! binary_op  {
    ($lua:ident, $ep:ident, $frame:ident, $frames:ident, $frame_count:ident, $l:ident, $op:tt, $r:ident, $opp:tt) => {
        match ($l, $r) {
            (Value::Number(left), Value::Number(right)) => (Value::Number(left $op right)),
            (Value::Integer(left), Value::Integer(right)) => (Value::Integer(left $op right)),
            (Value::Number(left), Value::Integer(right)) => (Value::Number(left $op right as f64)),
            (Value::Integer(left), Value::Number(right)) =>(Value::Number(left as f64 $op right)),
            (Value::String(left), Value::String(right)) => str_op_str!(left $op right $opp),
            (Value::String(left), Value::Integer(right)) => str_op_int!(left $op right $opp),
            (Value::Integer(left), Value::String(right)) => int_op_str!(left $op right $opp),
            (Value::String(left), Value::Number(right)) => str_op_num!(left $op right $opp),
            (Value::Number(left), Value::String(right)) => num_op_str!(left $op right $opp),
            (Value::Table(left), rr ) => {
                table_meta_op!($lua, $ep, $frame, $frames, $frame_count, left,  rr, $opp)
            },
            (Value::UserData(left), right) => {
                let er = right.to_error(); // just in case, cheap op
                match $lua.handle_userdata_binary_op($ep, left, MetaMethod::$opp, right) {
                    Ok(result) => result,
                    Err(_) => break Err(SiltError::ExpOpValueWithValue(
                        ValueTypes::UserData,
                        MetaMethod::$opp,
                        er
                    ))
                }
            },
            (ll,rr) => break Err(SiltError::ExpOpValueWithValue(ll.to_error(), MetaMethod::$opp, rr.to_error()))
        }
    };
}

#[allow(unused_macros)]
macro_rules! check_meta {
    ( $lua:ident, $i:tt, $op:expr) => {
        if let Some(table) = $lua.primative_meta_tables.get($i) {
            table.borrow()
        } else {
            $op
        }
    };
}

macro_rules! table_meta_op {
    ($lua:ident, $ep:ident, $frame:ident, $frames:ident, $frame_count:ident, $table:ident, $right:ident, $opp:tt) => {{
        let a = $table.borrow().by_meta_method(MetaMethod::$opp);
        match a {
            Ok(f) => {
                /*
                 * we push our metamethod onto the stack followed by the Table and the operand
                 * value.
                 * Notice in a regular call our frame_top is +1 of arity and our snapshot is -1 of
                 * arity, but here we dont do that. So... this should be tested better.
                 * This feels dirty, but I wrote this very sleep deprived and it works so this is a
                 * future me problem.
                 */
                $lua.push($ep, f);
                $lua.push($ep, Value::Table($table));
                // $lua.push($ep,$right);
                let arity: u8 = 1;
                let val = $lua.peekn($ep, arity);
                // println!(" we attempt to call {}", val);
                if let Value::Closure(c) = val {
                    const ARITY: usize = 2;
                    let frame_top = unsafe { $ep.ip.sub(ARITY) };
                    let new_frame = CallFrame::new(c.clone(), $lua.stack_count - ARITY, ARITY as u8, 0); // TODO using this opcode method means metamethods cant multireturn
                    $frames.push(new_frame);
                    $frame = $frames.last_mut().unwrap();
                    $frame.local_stack = frame_top;
                    $frame_count += 1;

                    $lua.print_stack();
                }
                // Value::Nil
                $right
            }
            Err(e) => break Err(e),
        }
    }};
}

type LuaResult = Result<ExVal, ErrorOut>;
type InnerUserData<'a> = Gc<'a, RefLock<UserDataWrapper>>;

/// Outcome of a [`Lua::hotswap`] / [`VM::hotswap`] call.
#[cfg(feature = "hot-swap")]
#[derive(Debug, PartialEq)]
pub enum HotswapResult {
    /// Old and new source are identical; no action was taken.
    NoChange,
    /// Root-level code changed; the entire source was recompiled and re-executed.
    RootChanged,
    /// One or more function bodies changed.  The compiled function objects have been
    /// replaced in the root function tree without re-executing root-level code, and every
    /// changed function that is bound as a top-level global closure has additionally been
    /// swapped *live* — the new body takes effect on the next call with all global/runtime
    /// state preserved (no `cycle()` required). Changed functions that aren't live global
    /// closures (locals, nested/instance methods) update in the root tree only and take
    /// effect on the next `cycle()` / re-instantiation.  The inner `Vec<String>` contains the
    /// names of every changed function (or `"anonymous"` for unnamed functions).
    FunctionChanged(Vec<String>),
}

/// Returns 1-indexed line numbers for every line that differs between `old` and
/// `new`.  An empty `Vec` means the two sources are identical.
#[cfg(feature = "hot-swap")]
fn find_changed_lines(old: &str, new: &str) -> Vec<usize> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let max_len = old_lines.len().max(new_lines.len());
    (0..max_len)
        .filter(|&i| {
            old_lines.get(i).copied().unwrap_or("") != new_lines.get(i).copied().unwrap_or("")
        })
        .map(|i| i + 1)
        .collect()
}

pub struct UDVec(pub Vec<WeakWrapper>);

unsafe impl Collect for UDVec {
    fn needs_trace() -> bool {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {
        // No GC references to trace
    }
}
pub struct Lua {
    arena: Arena<Rootable![VM<'_>]>,
}

impl<'gc> Lua {
    pub fn new() -> Self {
        let arena = Arena::<Rootable![VM<'_>]>::new(|mc| {
            // Gc::new(mc, RefLock::new(FunctionObject::new(None, false)))
            // GcRefLock
            VM::new(mc)
            // Gc::new(mc, RefLock::new(Value::Nil))
            // mc.alloc_many(256)
        });

        Self { arena }
    }

    pub fn new_with_standard() -> Self {
        let arena = Arena::<Rootable![VM<'_>]>::new(|mc| {
            let mut v = VM::new(mc);
            v.load_standard_library(mc);
            v
        });

        Self { arena }
    }

    pub fn run(&mut self, name: Option<&str>, code: &str, compiler: &mut Compiler) -> LuaResult {
        let out = self.arena.mutate_root(|mc, root| {
            match compiler.try_compile(mc, name, code) {
                Ok(f) => {
                    let res: LuaResult = root.borrow_mut().run(mc, Gc::new(mc, f));

                    // let res=root..run(mc, Gc::new(mc,f));
                    res
                }
                Err(er) => Err(er),
            }
        });
        out
    }

    pub fn compile(
        &mut self,
        name: Option<&str>,
        code: &str,
        compiler: &mut Compiler,
    ) -> LuaResult {
        self.arena
            .mutate_root(|mc, vm| match compiler.try_compile(mc, name, code) {
                Ok(f) => {
                    vm.borrow_mut().root = Gc::new(mc, f);
                    Ok(ExVal::Nil)
                }
                Err(err) => Err(err),
            })
    }

    pub fn cycle(&mut self) -> LuaResult {
        self.arena.mutate_root(|mc, vm| vm.borrow_mut().cycle(mc))
    }

    /// Hotswap Lua source code, detecting what changed between `old_source` and `new_source`
    /// and updating the VM accordingly with minimal disruption to runtime state.
    ///
    /// * If only function bodies changed, the root function object tree is updated with the
    ///   newly-compiled functions without re-executing root-level code, and any changed
    ///   top-level global functions are swapped live so the new bodies take effect on the
    ///   next call with all global state preserved — no `cycle()` needed.
    /// * If root-level code changed (or the diff spans multiple functions / the root scope),
    ///   the entire source is recompiled and executed – equivalent to calling [`run`](Self::run).
    /// * If the sources are identical, nothing happens.
    ///
    /// The `compiler` is reset between calls; callers should pass the same `Compiler` instance
    /// they used for the initial compilation.
    #[cfg(feature = "hot-swap")]
    pub fn hotswap(
        &mut self,
        name: Option<&str>,
        old_source: &str,
        new_source: &str,
        compiler: &mut Compiler,
    ) -> Result<HotswapResult, ErrorOut> {
        self.arena.mutate_root(|mc, vm| {
            vm.borrow_mut()
                .hotswap(mc, name, old_source, new_source, compiler)
        })
    }

    /// enter into the VM state to modify the VM directly
    pub fn enter<F, T>(&mut self, mut closure: F) -> T
    where
        F: for<'a> FnMut(&mut VM<'a>, &Mutation<'a>) -> T,
    {
        self.arena.mutate_root(move |mc, vm| {
            closure(vm, mc)
            // Ok(ExVal::Nil)

            // vm.borrow_mut().cycle(mc)
        })
    }

    /// Loads lua code into a callable function and returns a useable index to call it via
    /// vm.call(ref)
    pub fn load_fn(
        &mut self,
        compiler: &mut Compiler,
        name: Option<&str>,
        code: &str,
    ) -> Result<usize, ErrorOut> {
        self.arena
            .mutate_root(|mc, vm| vm.load_fn(mc, compiler, name, code))
    }

    /// call an internal function by index provided from the load function. Ideally call this after
    /// entering the VM context otherwise calling here will open and close the arena
    /// each time
    pub fn call(&mut self, name: Option<&str>, index: usize) -> LuaResult {
        self.call_with_params::<Vec<()>>(name, index, vec![])
        // Ok(ExVal::Nil)
    }

    /// call an internal function by index with parameters
    pub fn call_with_params<T>(&mut self, name: Option<&str>, index: usize, params: T) -> LuaResult
    where
        T: for<'e> ToLuaMulti<'e>,
    {
        self.arena
            .mutate_root(|mc, vm| vm.call_fn(mc, name, index, params))
        // rr
        // Ok(ExVal::Nil)
    }

    //     pub fn register<N>(&mut self, name: &str, function: N)
    //         where
    //             N: for<'a> Fn(&mut VM<'a>, &Mutation<'a>, Vec<Value<'a>>)-> Value<'a>,
    //     {
    //
    // // fn(&mut VM<'lua>, &Mutation<'lua>, Vec<Value<'lua>>) -> Value<'lua>
    //         self.arena
    //             .mutate_root( |mc, vm| {
    //                 // vm.register_native_function(mc, name, function);
    //                 vm.testy(mc, name);
    //             });
    //     }

    // fn fun<'a>(chunk: FunctionObject<'a>) -> FunctionObject<'a> {
    //     chunk
    // }
    //
    // pub fn run_chunk<'a>(&mut self, chunk: FunctionObject<'static>) -> LuaResult {
    //     self.arena.mutate_root(|mc, root| {
    //         let f = Self::fun(chunk);
    //         let res: LuaResult = root.borrow_mut().run(mc, Gc::new(mc, f));
    //         res
    //     });
    //     Ok(ExVal::Nil)
    // }
    // pub fn register(&mut self, name: &str, function: fn(&mut VM<'a>, &Mutation<'a>, Vec<Value<'a>>) -> Value<'a>)
    // {
    //         self.arena
    //             .mutate_root(|mc, vm| vm.register_native_function(mc, name, function))
    //
    // }
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct VM<'gc> {
    /// highest level frame or function object.
    root: Gc<'gc, FunctionObject<'gc>>,
    /// holds our current frame, our function object. At top level is a dupe of root
    body: Gc<'gc, FunctionObject<'gc>>,
    // compiler: Compiler<'lua>,
    // frames: Vec<CallFrame>,
    // dummy_frame: CallFrame,
    /** Instruction to be run at start of loop  */
    // ip: *const OpCode, // TODO usize vs *const OpCode, will rust optimize the same?
    // stack: Vec<Value>, // TODO fixed size array vs Vec, how much less overhead is there?
    stack: [Value<'gc>; 256],
    // stack_top: Gc<'lua,*mut Value<'lua>>,
    stack_count: usize,
    /** Next empty location */
    // stack_top: *mut Value,
    pub globals: Gc<'gc, RefLock<Table<'gc>>>, // TODO store strings as identifer usize and use that as key
    pub primative_meta_tables: Vec<Gc<'gc, RefLock<Table<'gc>>>>,
    // original CI code uses linked list, most recent closed upvalue is the first and links to previous closed values down the chain
    // allegedly performance of a linked list is heavier then an array and shifting values but is that true here or the opposite?
    // resizing a sequential array is faster then non sequential heap items, BUT since we'll USUALLY resolve the upvalue on the top of the list we're derefencing once to get our Upvalue vs an index lookup which is slightly slower.
    // TODO TLDR: benchmark this
    open_upvalues: Vec<Gc<'gc, RefLock<UpValue<'gc>>>>,
    // references: Vec<Reference>,
    // TODO should we store all strings in their own table/array for better equality checks? is this cheaper?
    // obj

    // TODO GC gray_stack
    // gray_stack: Vec<Value>,
    // TODO temporary solution to a hash id
    table_counter: RefCell<usize>,
    // a useful index to reference the current root table on a stack, used for userdata
    // table_op_index: usize,
    // meta_lookup: HashMap<String, MetaMethod>,
    // string_meta: Option<Gc<Table>>,
    pub userdata_registry: UserDataRegistry<'gc>,
    userdata_stack: Option<UDVec>,
    /// Used to quickly run in-VM functions externally
    external_functions: Vec<Gc<'gc, FunctionObject<'gc>>>,
}

pub(crate) struct Ephemeral<'a, 'g> {
    pub(crate) ip: *mut Value<'g>,
    pub(crate) mc: &'a Mutation<'g>,
}

impl<'a, 'g> Ephemeral<'a, 'g> {
    pub fn new(mc: &'a Mutation<'g>, ip: *mut Value<'g>) -> Self {
        Ephemeral { mc, ip }
    }
}

// #[allow(dead_code)]
// fn wrap<'gc, T: Collect>(mc: &Mutation<'gc>, value: T) -> ObjectPtr<'gc, T> {
//     Gc::new(mc, RefLock::new(value))
// }

// fn new_body<'a, 'b>(mc: &Mutation<'a>) -> Gc<'a, RefLock<FunctionObject<'b>>>
// where
//     'a: 'b,
// {
//     // Gc::new(mc, RefLock::new(FunctionObject::new(None, false)))
//     wrap_obj(mc, FunctionObject::new(None, false))
// }

impl<'gc> VM<'gc> {
    /** Create a new lua compiler and runtime */
    pub fn new(mc: &Mutation<'gc>) -> Self {
        // TODO try the hard way
        // force array to be 256 Values
        // let stack = unsafe {
        //     std::alloc::alloc(std::alloc::Layout::array::<Value>(256).unwrap()) as *mut [Value; 256]
        // };
        // let stack: [Value; 256] = [const { Value::Nil }; 256];
        // let mut arena = LuaArena::new(|mc| {
        //     Gc::new(mc, RefLock::new(FunctionObject::new(None, false)))
        // Gc::new(mc, RefLock::new(Value::Nil))
        // mc.alloc_many(256)
        // });

        // let mut arena = Arena::<Rootable![NodePtr<'_, i32>]>::new(|mc| {
        //     // Create a simple linked list with three links.
        //     //
        //     // 1 <-> 2 <-> 3 <-> 4

        //     let one = new_node(mc, 1);
        //     let two = new_node(mc, 2);
        //     let three = new_node(mc, 3);
        //     let four = new_node(mc, 4);

        //     node_join(mc, one, two);
        //     node_join(mc, two, three);
        //     node_join(mc, three, four);

        //     // We return the pointer to 1 as our root
        //     one
        // });

        let stack = [(); 256].map(|_| Value::default());
        // let stack_top = Gc::new(mc,RefLock::new( stack.as_mut_ptr() as *mut Value) );
        // let stack = vec![];
        // let stack_top = stack.as_ptr() as *mut Value;
        // let gtable  =RefLock::new(HashMap::new() );
        Self {
            // compiler: Compiler::new(),
            root: Gc::new(mc, FunctionObject::new(None, false)),
            body: Gc::new(mc, FunctionObject::new(None, false)),
            // dummy_frame: CallFrame::new(Rc::new(FunctionObject::new(None, false))),
            // frames: vec![],
            // ip: 0 as *const OpCode,
            // stack, //: unsafe { *stack },
            stack_count: 0,
            stack,
            // stack_top,
            globals: Gc::new(mc, RefLock::new(Table::new(0))), //Gc::new(mc, gtable),
            primative_meta_tables: vec![],
            open_upvalues: vec![],
            table_counter: RefCell::new(1),
            userdata_registry: UserDataRegistry::new(),
            userdata_stack: Some(UDVec(vec![])),
            external_functions: vec![],
        }
    }

    /// set a built Function Object as root and run it
    pub fn run(
        &mut self,
        mc: &Mutation<'gc>,
        object: Gc<'gc, FunctionObject<'gc>>,
    ) -> Result<ExVal, ErrorOut> {
        self.execute(mc, object)
    }

    /// compile and run lua once
    pub fn build_and_run(
        &mut self,
        mc: &Mutation<'gc>,
        name: Option<&str>,
        code: &str,
        compiler: &mut Compiler,
    ) -> Result<ExVal, ErrorOut> {
        match compiler.try_compile(mc, name, code) {
            Ok(f) => {
                let fun = Gc::new(mc, f);
                self.run(mc, fun)
            }
            Err(er) => Err(er),
        }
    }

    /// run through the full program again, keeping previous state. This will redeclare top level
    /// code too. Ideally you will want to directly run a function via call_by_index after load_fn
    /// or store_fn
    pub fn cycle(&mut self, mc: &Mutation<'gc>) -> Result<ExVal, ErrorOut> {
        self.execute(mc, self.root)
    }

    /// Hotswap Lua source code with minimal VM disruption.
    ///
    /// Computes the diff between `old_source` and `new_source`.  Uses the actual compiler
    /// output to determine which functions changed: each `CLOSURE` instruction in the
    /// compiled root chunk is examined; its start line comes from the chunk's location table
    /// and its end line from the last instruction of the function's own chunk.  Functions
    /// whose line range overlaps with the diff are treated as changed and kept from the new
    /// compilation; all other function constants are restored from the original root so that
    /// GC identity is preserved and only what actually changed is updated.
    ///
    /// - **All diff lines inside function bodies** → `FunctionChanged(names)`.  The root is
    ///   updated but root-level code is NOT re-executed, preserving global state.  Call
    ///   [`cycle`](Self::cycle) to re-register function globals when ready.
    /// - **Any diff line outside every function body** → `RootChanged`.  The full source is
    ///   recompiled and re-executed immediately.
    /// - **Identical sources** → `NoChange`.
    ///
    /// This approach handles named, anonymous, and nested functions correctly because it
    /// relies on the actual compiler output rather than a secondary parse pass.
    #[cfg(feature = "hot-swap")]
    pub fn hotswap(
        &mut self,
        mc: &Mutation<'gc>,
        name: Option<&str>,
        old_source: &str,
        new_source: &str,
        compiler: &mut Compiler,
    ) -> Result<HotswapResult, ErrorOut> {
        // 1. Find the set of changed lines (1-indexed).  An empty set means no change.
        let changed_lines = find_changed_lines(old_source, new_source);
        if changed_lines.is_empty() {
            return Ok(HotswapResult::NoChange);
        }

        // 2. Compile the new source fully using the real parser/compiler.
        let mut new_root = compiler.try_compile(mc, name, new_source)?;

        // 3. Scan every CLOSURE instruction in the new root chunk.
        //
        //    For each compiled function we use:
        //      • `fn_gc.start_line` – the line of the `function` keyword.
        //      • `fn_gc.end_line`   – the line of the matching `end` keyword, captured
        //        directly from the lexer token in `block()` during compilation.  This is
        //        more precise than using the last instruction's line, since the `end`
        //        keyword itself never generates bytecode.
        //
        //    A function "overlaps the diff" when at least one changed line falls within
        //    [start_line, end_line].  Nested / anonymous functions are handled because
        //    their containing root-level function's span covers them.
        let mut changed_fn_names: Vec<String> = vec![];
        // (start_line, end_line) for every function that overlaps the diff.
        let mut covered_ranges: Vec<(usize, usize)> = vec![];
        // Constant indices of root-level functions whose source did NOT change; they will
        // be restored from `self.root` to preserve GC object identity.
        let mut unchanged_indices: Vec<usize> = vec![];
        // (name, new FunctionObject) for every CHANGED root-level *named* function, used by
        // the stage-1 live swap below to update the matching global closure in place without
        // re-running root-level code. Anonymous functions are skipped (no global to bind).
        let mut changed_globals: Vec<(String, Gc<'gc, FunctionObject<'gc>>)> = vec![];

        for opcode in new_root.chunk.code.iter() {
            if let crate::code::OpCode::CLOSURE { constant } = opcode {
                let k = *constant as usize;
                if let crate::value::Value::Function(fn_gc) = new_root.chunk.get_constant(*constant)
                {
                    let fn_start_line = fn_gc.start_line;
                    // Prefer the compiler-recorded `end` keyword line for precise detection.
                    // Fall back to the last instruction's line if end_line wasn't set.
                    let fn_end_line = if fn_gc.end_line > 0 {
                        fn_gc.end_line
                    } else {
                        fn_gc.chunk.last_line()
                    };

                    // A function overlaps the diff when any changed line falls in its span.
                    let overlaps = changed_lines
                        .iter()
                        .any(|&line| fn_start_line <= line && line <= fn_end_line);

                    if overlaps {
                        let fn_name = fn_gc
                            .name
                            .clone()
                            .unwrap_or_else(|| "anonymous".to_string());
                        changed_fn_names.push(fn_name.clone());
                        covered_ranges.push((fn_start_line, fn_end_line));
                        // Only named functions can be bound as a global and swapped live.
                        if fn_gc.name.is_some() {
                            changed_globals.push((fn_name, *fn_gc));
                        }
                    } else {
                        unchanged_indices.push(k);
                    }
                }
            }
        }

        // 4. Decide whether root-level code changed. A changed line forces a full
        //    reload if either:
        //      (a) it lies outside every changed function's span, OR
        //      (b) the ROOT chunk carries genuine root-level code at that line —
        //          even if a function span also covers it. This catches edits on
        //          a line *shared* between a function and root code (e.g. the
        //          unformatted `counter = 5 function tick() … end`), which the
        //          span check alone would silently mask as a function-only change.
        //    Function bodies live in their own chunks, so the only root-chunk
        //    instructions are root statements plus the function-definition
        //    machinery (CLOSURE / REGISTER_UPVALUE / DEFINE_GLOBAL); we exclude
        //    that machinery so a pure signature edit still counts as function-scope.
        let mut root_code_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (i, opcode) in new_root.chunk.code.iter().enumerate() {
            match opcode {
                crate::code::OpCode::CLOSURE { .. }
                | crate::code::OpCode::REGISTER_UPVALUE { .. }
                | crate::code::OpCode::DEFINE_GLOBAL { .. } => {}
                _ => {
                    let (line, _) = new_root.chunk.get_loc(i);
                    root_code_lines.insert(line);
                }
            }
        }
        let root_level_changed = changed_lines.iter().any(|&line| {
            root_code_lines.contains(&line)
                || !covered_ranges
                    .iter()
                    .any(|&(start, end)| start <= line && line <= end)
        });

        if root_level_changed || changed_fn_names.is_empty() {
            // A root change is a FULL RESET: wipe all VM state and re-run the new
            // program from scratch, exactly as a freshly constructed VM would —
            // no stale globals/userdata from the previous version survive.
            self.globals = Gc::new(mc, RefLock::new(Table::new(0)));
            self.stack_count = 0;
            if let Some(u) = &mut self.userdata_stack {
                u.0.clear();
            }
            self.load_standard_library(mc);
            let func = Gc::new(mc, new_root);
            self.root = func;
            self.execute(mc, func)?;
            return Ok(HotswapResult::RootChanged);
        }

        // 5. All changes are inside function bodies.
        //    Restore unchanged function constants from the original root so that only the
        //    actually-changed functions differ; this preserves GC object identity and avoids
        //    unnecessarily replacing live function references.  The constant indices align
        //    because we compiled the full source (same function order = same constant order).
        let old_constants_len = self.root.chunk.constants_len();
        for k in unchanged_indices {
            if k < old_constants_len {
                let orig = self.root.chunk.copy_constant(k as u8);
                new_root.chunk.patch_constant(k, orig);
            }
        }

        // 6. Install the patched root without re-executing root-level code, thereby
        //    preserving all global state.  The patched root keeps the VM's notion of the
        //    program consistent (used by any later `cycle()` and as the source for nested /
        //    not-yet-instantiated functions).
        self.root = Gc::new(mc, new_root);

        // 7. STAGE 1 — live-apply the change. For each changed *named* function that is
        //    currently bound as a top-level global closure, replace that global with a fresh
        //    closure wrapping the new code while REUSING the existing upvalue cells. Because
        //    top-level functions are invoked by global-name lookup, every subsequent call
        //    picks up the new body immediately — no `cycle()`, so all global/game state is
        //    preserved. Functions that aren't live global closures (locals, nested methods)
        //    are left to the patched root tree; truly surgical nested/instance swaps are
        //    stage 2 (function-slot indirection).
        for (fn_name, new_fn) in changed_globals {
            // Capture the live closure's upvalue cells, if this name is a global closure.
            let old_upvalues = match self.globals.borrow().get(fn_name.as_str()) {
                Some(Value::Closure(old)) => Some(old.upvalues.clone()),
                _ => None,
            };
            let Some(old_upvalues) = old_upvalues else {
                continue; // not a live global closure — nothing to swap in place
            };
            // Reuse the existing cells only when the capture layout is unchanged (the
            // body-only edit case). If the new body captures a different number of
            // upvalues we can't safely rebind here, so leave the live global as-is; the
            // patched root still carries the new code for a future full cycle.
            let upvalues = if new_fn.upvalue_count as usize == old_upvalues.len() {
                old_upvalues
            } else if new_fn.upvalue_count == 0 {
                vec![]
            } else {
                continue;
            };
            let new_closure = Gc::new(mc, Closure::new(new_fn, upvalues));
            self.globals
                .borrow_mut(mc)
                .set(Value::String(fn_name), Value::Closure(new_closure));
        }

        Ok(HotswapResult::FunctionChanged(changed_fn_names))
    }

    /// Identical to run (mostly), set a built Function Object as root and run it
    pub fn execute(
        &mut self,
        mc: &Mutation<'gc>,
        object: Gc<'gc, FunctionObject<'gc>>,
    ) -> Result<ExVal, ErrorOut> {
        // TODO param is a reference of &'a
        // self.ip = object.chunk.code.as_ptr();
        // frame.ip = object.chunk.code.as_ptr();
        // frame.slots = self.stack ???
        // let rstack = self.stack.as_ptr();
        #[cfg(feature = "dev-out")]
        object.chunk.print_chunk(&None);
        let mut ep = Ephemeral::new(mc, self.stack.as_mut_ptr() as *mut Value);
        self.body = object;
        // *root = new_body(mc, object.clone());
        let closure = Gc::new(mc, Closure::new(object, vec![]));

        let mut frame = CallFrame::new(closure, 0, 0, 0);
        frame.ip = object.chunk.code.as_ptr();
        frame.local_stack = ep.ip;
        // frame.stack.resize(256, Value::Nil); // TODO
        self.push(&mut ep, Value::Function(object)); // TODO this needs to store the function object itself somehow, RC?
        let frames = vec![frame];
        self.process(&mut ep, frames)
    }

    /// dump all newest userdata as weak references but keep atomic strong references within the lua
    /// vm. Ideally garbage collected
    pub fn drain_userdata(&mut self) -> Vec<WeakWrapper> {
        match &mut self.userdata_stack {
            Some(u) => std::mem::take(&mut u.0),
            None => vec![],
        }
    }

    /// compile lua code and store as function, return callable index
    pub fn load_fn<'a>(
        &mut self,
        mc: &'a Mutation<'gc>,
        compiler: &mut Compiler,
        name: Option<&str>,
        code: &str,
    ) -> Result<usize, ErrorOut> {
        match compiler.try_compile(mc, name, code) {
            Ok(f) => {
                let fun = Gc::new(mc, f);
                Ok(self.store_fn(fun))
            }
            Err(er) => Err(er),
        }
    }

    /// insert a function object and return the callable index
    pub fn store_fn(&mut self, o: Gc<'gc, FunctionObject<'gc>>) -> usize {
        let u = self.external_functions.len();
        self.external_functions.push(o);
        u
    }

    /// push value to stack
    pub(crate) fn push(&mut self, ep: &mut Ephemeral<'_, 'gc>, value: Value<'gc>) {
        VM::push_raw(ep, value);
        self.stack_count += 1;
    }

    /// push value to stack without stack adjustment (convenience for mutable reference hiccups)
    fn push_raw<'e>(ep: &mut Ephemeral<'e, 'gc>, value: Value<'gc>) {
        devout!(" | push: {}", value);
        unsafe { ep.ip.write(value) };
        ep.ip = unsafe { ep.ip.add(1) };
    }

    pub(crate) fn push_nils(&mut self, ep: &mut Ephemeral<'_, 'gc>, amount: usize) {
        devout!(" | push nils x{}", amount);
        // const NIL: u8 = unsafe{ std::mem::transmute::<Value, u8>(Value::Nil)};
        // ep.ip = unsafe {
        //     ep.ip.write_bytes(NIL, amount);
        //     ep.ip.add(amount)
        // };
        for _ in 0..amount {
            unsafe { ep.ip.write(Value::Nil) };
            ep.ip = unsafe { ep.ip.add(1) };
        }
        self.stack_count += amount;
    }

    pub(crate) fn pushn(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        values: &[Value<'gc>],
        need: usize,
        is_rev: bool,
    ) {
        devout!(" | push_n: values x {}, need {}", values.len(), need);
        // for v in values.iter() {
        //     println!("we have {}", v);
        // }
        let _n = values.len();
        let c = need;
        if is_rev {
            // TODO this is sloppy make this DRYer
            let mut vv = values.iter().rev();
            for _ in 0..c {
                // TODO pushing nil is stupid, right? popping always writes nils so we shouldnt leak?
                // let v= match vv.next(){
                //     Some(v)=>v,
                //     None=>Value::Nil
                // }

                if let Some(v) = vv.next() {
                    devout!("pushn -> {}", v);
                    unsafe { ep.ip.write(v.clone()) };
                };
                ep.ip = unsafe { ep.ip.add(1) };
            }
        } else {
            let mut vv = values.into_iter();
            for _ in 0..c {
                // TODO pushing nil is stupid, right? popping always writes nils so we shouldnt leak?
                // let v= match vv.next(){
                //     Some(v)=>v,
                //     None=>Value::Nil
                // }

                if let Some(v) = vv.next() {
                    devout!("pushn -> {}", v);
                    unsafe { ep.ip.write(v.clone()) };
                };
                ep.ip = unsafe { ep.ip.add(1) };
            }
        }

        self.stack_count += need;
    }

    #[allow(dead_code)]
    fn reserve(&mut self, ep: &mut Ephemeral<'_, 'gc>) -> *mut Value<'gc> {
        self.stack_count += 1;
        let old = ep.ip;
        ep.ip = unsafe { ep.ip.add(1) };
        old
    }

    /** pop N number of values from stack */
    fn popn_drop(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: u8) {
        unsafe { ep.ip = ep.ip.sub(n as usize) };
        self.stack_count -= n as usize;
    }

    #[cfg(feature = "dev-out")]
    fn print_upvalues(&self) {
        self.open_upvalues.iter().enumerate().for_each(|(i, up)| {
            // let m=unsafe{};
            println!("{}:{}", i, unsafe { up.as_ptr().read() }); // TODO can we make this safer?
        });
    }

    fn close_n_upvalues(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: u8) {
        #[cfg(feature = "dev-out")]
        self.print_upvalues();
        // remove n from end of list
        if n > 1 {
            self.open_upvalues
                .drain(self.open_upvalues.len() - n as usize..)
                .rev()
                .for_each(|up| {
                    let mut upvalue = up.borrow_mut(ep.mc);
                    upvalue.close_around(unsafe { ep.ip.replace(Value::Nil) });
                });
            unsafe { ep.ip = ep.ip.sub(n as usize) };
        } else {
            let upvalue = self.open_upvalues.pop().unwrap();

            upvalue.borrow_mut(ep.mc).close_around(self.pop(ep));
        }
    }

    /// Close every open upvalue that points at or above `last` (the returning
    /// frame's base slot) and drop it from `open_upvalues`. Closing copies the
    /// captured value off the soon-to-be-reclaimed stack into the UpValue's own
    /// heap cell, so surviving closures keep seeing the right value after the
    /// frame is gone. We must `borrow_mut` the actual Gc cell here — reading a
    /// copy of the UpValue and closing that leaves the real cell open, pointing
    /// at dead stack memory.
    fn close_upvalues_by_return(&mut self, mc: &Mutation<'gc>, last: *mut Value<'gc>) {
        #[cfg(feature = "dev-out")]
        self.print_upvalues();
        let mut i = 0;
        while i < self.open_upvalues.len() {
            let loc = self.open_upvalues[i].borrow().location;
            if loc >= last {
                let up = self.open_upvalues.remove(i);
                up.borrow_mut(mc).close();
            } else {
                i += 1;
            }
        }
    }

    /** pop and return top of stack */
    fn pop(&mut self, ep: &mut Ephemeral<'_, 'gc>) -> Value<'gc> {
        self.stack_count -= 1;
        unsafe { ep.ip = ep.ip.sub(1) };
        let v = unsafe { ep.ip.replace(Value::Nil) };
        // TODO is there a way to read without segfaulting?
        // We'd have to list the value to be forgotten, but is this even faster?
        // let v = unsafe { ep.ip.read() };
        devout!(" | pop: {}", v);
        v
    }

    fn pop_offset(&mut self, ep: &mut Ephemeral<'_, 'gc>, offset: usize) -> Value<'gc> {
        self.stack_count -= offset;
        for _ in 1..offset {
            unsafe { ep.ip = ep.ip.sub(1) };
            unsafe { ep.ip.replace(Value::Nil) };
        }
        unsafe { ep.ip = ep.ip.sub(1) };
        unsafe { ep.ip.replace(Value::Nil) }
    }

    // TODO can we make this faster with slices? can we slice a pointer? 🤔
    fn popn(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: u8) -> Vec<Value<'gc>> {
        // println!("popn: {}", n);
        let mut values = vec![];
        for _ in 0..n {
            self.stack_count -= 1;
            unsafe { ep.ip = ep.ip.sub(1) };
            let v = unsafe { ep.ip.replace(Value::Nil) };
            values.push(v);
        }
        // TODO inefficient just make it this way
        values.reverse();
        values
    }

    #[allow(dead_code)]
    fn safe_pop(&mut self) -> Value<'gc> {
        // let v3 = take(&mut self.stack[3]);
        // println!("we took {}", v3);
        // let v0 = take(&mut self.stack[self.stack_count - 1]);
        // println!("we took {}", v0);
        // let ve = v0.clone();
        // std::mem::forget(v3);
        // drop(v0);
        // println!("we took {}", ve);
        // self.print_raw_stack();
        // core::ptr::read()

        take(&mut self.stack[self.stack_count - 1])

        // for i in self.stack.iter_mut().enumerate() {
        //     *i = Value::Nil;
        // }
    }

    /** Dangerous!  */
    #[allow(dead_code)]
    fn read_top(&self, ep: &mut Ephemeral<'_, 'gc>) -> Value<'gc> {
        unsafe { ep.ip.sub(1).read() }
    }

    /** Safer but clones! */
    fn duplicate(&self, ep: &mut Ephemeral<'_, 'gc>) -> Value<'gc> {
        unsafe { (*ep.ip.sub(1)).clone() }
    }

    /** Look and get immutable reference to top of stack */
    fn peek(&self, ep: &mut Ephemeral<'_, 'gc>) -> &Value<'gc> {
        // self.stack.last()
        unsafe { &*ep.ip.sub(1) }
    }

    /** Look and get mutable reference to top of stack */
    fn peek_mut(&self, ep: &mut Ephemeral<'_, 'gc>) -> &mut Value<'gc> {
        unsafe { &mut *ep.ip.sub(1) }
    }

    fn grab(&self, ep: &mut Ephemeral<'_, 'gc>, n: usize) -> &Value<'gc> {
        unsafe { &*ep.ip.sub(n) }
    }

    // fn grab_mut(&self, ep: &mut Ephemeral<'_, 'gc>, n: usize) -> &mut Value<'gc> {
    //     unsafe { &mut *ep.ip.sub(n) }
    // }

    /** Look down N amount of stack and return immutable reference */
    fn peekn(&self, ep: &mut Ephemeral<'_, 'gc>, n: u8) -> &Value<'gc> {
        // unsafe { *ep.ip.sub(n as usize) }
        // &self.stack[self.stack.len() - n as usize]
        unsafe { &*ep.ip.sub((n as usize) + 1) }
    }

    // pub fn evaluate(&mut self, source: &str) -> FunctionObject<'lua> {
    //     self.compiler.compile(source.to_owned())
    // }

    /// The actual crawl through the entire root function object until it completes. This does not
    /// clear state on subsequent re-runs so variables could get redefined without any checks ( if
    /// x~=nil then x=1 end for instance )
    fn process(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        mut frames: Vec<CallFrame<'gc>>,
    ) -> Result<ExVal, ErrorOut> {
        // let mut last = Value::Nil; // TODO temporary for testing
        // let stack_pointer = self.stack.as_mut_ptr();
        // let mut dummy_frame = CallFrame::new(Rc::new(FunctionObject::new(None, false)), 0);
        let mut frame = frames.last_mut().unwrap();
        let mut frame_count = 1;
        // monkey patch for variadic as an argument since CALL op tries to count varibles used
        // body.chunk.print_chunk(None);
        #[cfg(feature = "dev-out")]
        let mut step_count = 0;
        let results: Result<ExVal, SiltError> = loop {
            let instruction = frame.current_instruction();

            // devout!("ip: {:p} | {}", self.ip, instruction);

            #[cfg(feature = "dev-out")]
            {
                step_count += 1;
                println!("[ {step_count} ]============");
            }
            devout!(" | {}", instruction);

            // TODO how much faster would it be to order these ops in order of usage, does match hash? probably.
            match instruction {
                OpCode::RETURN(c) => {
                    let count = *c;
                    frame_count -= 1;
                    if frame_count <= 0 {
                        let out = self.pop(ep).into();
                        // if self.stack_count <= 1 {
                        //     return Ok(ExVal::Nil);
                        // }
                        // let out: ExVal = self.safe_pop().into();
                        return Ok(out);
                    }

                    devout!(
                        "=========  ask for {} capable of {}",
                        frame.multi_return,
                        count
                    );
                    let multi_return = frame.multi_return;
                    // if  || frame.need>1 {
                    if multi_return > 1 && count > 1 {
                        // TODO seriously stupid to make a Vec and then slice it
                        let vres = &self.popn(ep, count);

                        // TODO this paragraph is a dupe of the one below, i hate this whole logic
                        // segment. Pushing stack values to a new vec, reversing it, then iterating
                        // one by one back on to the stack but further down? Literally wasteful!
                        // so we will rewrite eventually

                        // Truncate to the original function slot (== local_stack for
                        // plain calls, but below it for variadic calls where the
                        // fixed params were copied above the overflow). Deriving it
                        // from the snapshot reclaims that overflow + copies too.
                        let snapshot = frame.stack_snapshot;
                        ep.ip = unsafe { self.stack.as_mut_ptr().add(snapshot) };
                        self.close_upvalues_by_return(ep.mc, ep.ip);
                        devout!("stack top {}", unsafe { &*ep.ip });
                        self.stack_count = snapshot;
                        frames.pop();
                        frame = frames.last_mut().unwrap();
                        devout!("next instruction {}", frame.current_instruction());
                        #[cfg(feature = "dev-out")]
                        self.print_stack();

                        self.pushn(ep, vres, multi_return as usize, false);
                    } else {
                        let res = if count > 1 {
                            self.pop_offset(ep, count as usize)
                        } else {
                            self.pop(ep)
                        };

                        // Truncate to the original function slot (== local_stack for
                        // plain calls, but below it for variadic calls where the
                        // fixed params were copied above the overflow). Deriving it
                        // from the snapshot reclaims that overflow + copies too.
                        let snapshot = frame.stack_snapshot;
                        ep.ip = unsafe { self.stack.as_mut_ptr().add(snapshot) };
                        self.close_upvalues_by_return(ep.mc, ep.ip);
                        devout!("stack top {}", unsafe { &*ep.ip });
                        self.stack_count = snapshot;
                        frames.pop();
                        frame = frames.last_mut().unwrap();
                        devout!("next instruction {}", frame.current_instruction());
                        // println!("yeah push {}", res);
                        self.push(ep, res);
                        #[cfg(feature = "dev-out")]
                        self.print_stack();

                        // if frame.need > 1 {
                        //     for _ in 1..frame.need {
                        //         self.push(ep, Value::Nil);
                        //     }
                        // }
                    }

                    // frame.need = 1;

                    // println!("<< {}", self.pop());
                    // match self.pop() {
                    //     Some(v) => return Ok(v),
                    //     None => return Ok(last),
                    // }
                }
                OpCode::CONSTANT { constant } => {
                    let value = Self::get_chunk(frame).get_constant(*constant);
                    self.push(ep, value.clone());
                    // match value {
                    //     Value::Number(f) => self.push(*f),
                    //     Value::Integer(i) => self.push(*i as f64),
                    //     _ => {}
                    // }
                }
                OpCode::DEFINE_GLOBAL { constant } => {
                    let value = self.body.chunk.get_constant(*constant);
                    if let Value::String(s) = value {
                        // devout!("\"{}\"", _);
                        // DEV inline pop due to self lifetime nonsense
                        self.stack_count -= 1;
                        unsafe { ep.ip = ep.ip.sub(1) };
                        let v = unsafe { ep.ip.read() };

                        // let v = self.pop();
                        // self.globals.borrow_mut(ep.mc).set::< K:Value<'gc>, V:Value<'gc>>(s.into(), v);
                        self.globals.borrow_mut(ep.mc).set(s, v);
                    } else {
                        break Err(SiltError::VmCorruptConstant);
                    }
                }

                // TODO does this need to exist?
                OpCode::SET_GLOBAL { constant } => {
                    let value = Self::get_chunk(frame).get_constant(*constant);
                    // let value = self.body.chunk.get_constant(*constant);
                    // devout!("ident: {}", value);
                    if let Value::String(s) = value {
                        devout!("\"{}\"", s);
                        let v = self.duplicate(ep);
                        // TODO we could take, expr statements send pop, this is a hack of sorts, ideally the compiler only sends a pop for nonassigment
                        // alternatively we can peek the value, that might be better to prevent side effects
                        // do we want expressions to evaluate to a value? probably? is this is ideal for implicit returns?

                        // if let Some(_) = self.globals.get(&**s) {
                        //     self.globals.insert(s.to_string(), v);
                        // } else {
                        //     self.globals.insert(s.to_string(), v);
                        // }
                        // devout!("set original: {}", value);
                        self.globals.borrow_mut(ep.mc).set(s, v);
                    } else {
                        // devout!("0SET_GLOBAL: {}", value);
                        #[cfg(feature = "dev-out")]
                        self.body.chunk.print_constants();
                        break Err(SiltError::VmCorruptConstant);
                    }
                }
                OpCode::GET_GLOBAL { constant } => {
                    let value = Self::get_chunk(frame).get_constant(*constant);
                    // devout!("ident: {}", value);
                    // println!(" we have keys {}",self.globals.borrow().list_keys());
                    if let Value::String(s) = value {
                        devout!("\"{}\"", s);

                        if let Some(v) = self.globals.borrow_mut(ep.mc).get(s) {
                            self.push(ep, v.clone());
                        } else {
                            self.push(ep, Value::Nil);
                        }
                    } else {
                        break Err(SiltError::VmCorruptConstant);
                    }
                }
                OpCode::SET_LOCAL { index } => {
                    let value = self.duplicate(ep);
                    // frame.stack[*index as usize] = value;
                    frame.set_val(*index, value)
                }
                OpCode::GET_LOCAL { index } => {
                    #[cfg(feature = "dev-out")]
                    {
                        println!("before {}", self.stack_count);
                        frame.print_local_stack();
                    }
                    // For variadic functions the frame base already sits past the
                    // variadic range (the fixed params are copied above the
                    // overflow at call time), so locals are addressed uniformly.
                    self.push(ep, frame.get_val(*index).clone());

                    #[cfg(feature = "dev-out")]
                    {
                        println!("after {}", self.stack_count);
                        frame.print_local_stack();
                    }
                    // self.push(frame.stack[*index as usize].clone());
                    // TODO ew cloning, is our cloning optimized yet?
                    // TODO also we should convert from stack to register based so we can use the index as a reference instead
                }
                OpCode::VARARG { is_arg, count } => {
                    // `nextra` = how many variadic overflow values this call actually
                    // received (call arity minus the number of fixed params).
                    let numfixed = frame.function.get_variadic();
                    let nextra = frame.call_arity.saturating_sub(numfixed);

                    // When spread as a call/return argument we forward every overflow
                    // value; otherwise the compiler tells us how many slots to fill
                    // (e.g. `local a = ...` wants exactly one, padding nils if short).
                    let val_count = if *is_arg { nextra } else { *count };
                    let take = val_count.min(nextra);

                    #[cfg(feature = "dev-out")]
                    {
                        println!("before {}", self.stack_count);
                        frame.print_local_stack();
                    }

                    let raw = frame.get_varargs(nextra);
                    self.pushn(ep, &raw[..take as usize], take as usize, false);
                    if val_count > nextra {
                        // not enough overflow values to satisfy the requested count
                        self.push_nils(ep, (val_count - nextra) as usize);
                    }

                    #[cfg(feature = "dev-out")]
                    {
                        println!("after {}", self.stack_count);
                        frame.print_local_stack();
                    }
                }
                OpCode::NEED(_) => {}
                OpCode::DEFINE_LOCAL { constant: _ } => todo!(),
                OpCode::ADD => binary_op_push!(self, ep, frame, frames, frame_count, +, Add),
                OpCode::SUB => binary_op_push!(self, ep, frame, frames, frame_count, -, Sub),
                OpCode::MULTIPLY => binary_op_push!(self, ep, frame, frames, frame_count, *, Mul),
                OpCode::DIVIDE => {
                    let right = self.pop(ep);
                    let left = self.pop(ep);

                    match (left, right) {
                        (Value::Number(left), Value::Number(right)) => {
                            self.push(ep, Value::Number(left / right))
                        }
                        (Value::Integer(left), Value::Integer(right)) => {
                            self.push(ep, Value::Number(left as f64 / right as f64))
                        }
                        (Value::Number(left), Value::Integer(right)) => {
                            self.push(ep, Value::Number(left / right as f64))
                        }
                        (Value::Integer(left), Value::Number(right)) => {
                            self.push(ep, Value::Number(left as f64 / right))
                        }
                        (Value::Table(table), rr) => {
                            let v = table_meta_op!(
                                self,
                                ep,
                                frame,
                                frames,
                                frame_count,
                                table,
                                rr,
                                Div
                            );
                            self.push(ep, v);
                        }
                        (l, r) => {
                            break Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                MetaMethod::Div,
                                r.to_error(),
                            ))
                        }
                    }
                }

                OpCode::MODULUS => {
                    let right = self.pop(ep);
                    let left = self.pop(ep);
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => {
                            if b == 0 {
                                break Err(SiltError::ExpOpValueWithValue(
                                    Value::Integer(a).to_error(),
                                    MetaMethod::Mod,
                                    Value::Integer(b).to_error(),
                                ));
                            }
                            // Lua's `%` is floored: the result takes the sign of
                            // the divisor (Rust's `%` is truncated toward zero).
                            let r = a % b;
                            let m = if r != 0 && (r < 0) != (b < 0) { r + b } else { r };
                            self.push(ep, Value::Integer(m));
                        }
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(ep, Value::Number(a - (a / b).floor() * b))
                        }
                        (Value::Number(a), Value::Integer(b)) => {
                            let b = b as f64;
                            self.push(ep, Value::Number(a - (a / b).floor() * b))
                        }
                        (Value::Integer(a), Value::Number(b)) => {
                            let a = a as f64;
                            self.push(ep, Value::Number(a - (a / b).floor() * b))
                        }
                        (Value::Table(table), rr) => {
                            let v = table_meta_op!(
                                self, ep, frame, frames, frame_count, table, rr, Mod
                            );
                            self.push(ep, v);
                        }
                        (l, r) => {
                            break Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                MetaMethod::Mod,
                                r.to_error(),
                            ))
                        }
                    }
                }
                OpCode::POWER => {
                    // `^` always yields a float, per Lua.
                    let right = self.pop(ep);
                    let left = self.pop(ep);
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => {
                            self.push(ep, Value::Number((a as f64).powf(b as f64)))
                        }
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(ep, Value::Number(a.powf(b)))
                        }
                        (Value::Number(a), Value::Integer(b)) => {
                            self.push(ep, Value::Number(a.powf(b as f64)))
                        }
                        (Value::Integer(a), Value::Number(b)) => {
                            self.push(ep, Value::Number((a as f64).powf(b)))
                        }
                        (Value::Table(table), rr) => {
                            let v = table_meta_op!(
                                self, ep, frame, frames, frame_count, table, rr, Pow
                            );
                            self.push(ep, v);
                        }
                        (l, r) => {
                            break Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                MetaMethod::Pow,
                                r.to_error(),
                            ))
                        }
                    }
                }
                OpCode::FLOOR_DIVIDE => {
                    let right = self.pop(ep);
                    let left = self.pop(ep);
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => {
                            if b == 0 {
                                break Err(SiltError::ExpOpValueWithValue(
                                    Value::Integer(a).to_error(),
                                    MetaMethod::IDiv,
                                    Value::Integer(b).to_error(),
                                ));
                            }
                            // floored integer division (rounds toward -inf),
                            // consistent with the floored `%` above.
                            let q = a / b;
                            let r = a % b;
                            let q = if r != 0 && (r < 0) != (b < 0) { q - 1 } else { q };
                            self.push(ep, Value::Integer(q));
                        }
                        (Value::Number(a), Value::Number(b)) => {
                            self.push(ep, Value::Number((a / b).floor()))
                        }
                        (Value::Number(a), Value::Integer(b)) => {
                            self.push(ep, Value::Number((a / b as f64).floor()))
                        }
                        (Value::Integer(a), Value::Number(b)) => {
                            self.push(ep, Value::Number((a as f64 / b).floor()))
                        }
                        (Value::Table(table), rr) => {
                            let v = table_meta_op!(
                                self, ep, frame, frames, frame_count, table, rr, IDiv
                            );
                            self.push(ep, v);
                        }
                        (l, r) => {
                            break Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                MetaMethod::IDiv,
                                r.to_error(),
                            ))
                        }
                    }
                }
                // Bitwise binary ops. Operands must have an integer representation
                // (Lua); tables are not yet dispatched to __band/etc and error.
                OpCode::BIT_AND
                | OpCode::BIT_OR
                | OpCode::BIT_XOR
                | OpCode::SHIFT_LEFT
                | OpCode::SHIFT_RIGHT => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    match (bit_int(&l), bit_int(&r)) {
                        (Some(a), Some(b)) => {
                            let res = match instruction {
                                OpCode::BIT_AND => a & b,
                                OpCode::BIT_OR => a | b,
                                OpCode::BIT_XOR => a ^ b,
                                OpCode::SHIFT_LEFT => lua_shl(a, b),
                                _ => lua_shr(a, b),
                            };
                            self.push(ep, Value::Integer(res));
                        }
                        (None, _) => break Err(SiltError::ExpInvalidBitwise(l.to_error())),
                        (_, None) => break Err(SiltError::ExpInvalidBitwise(r.to_error())),
                    }
                }
                OpCode::BIT_NOT => {
                    let v = self.pop(ep);
                    match bit_int(&v) {
                        Some(a) => self.push(ep, Value::Integer(!a)),
                        None => break Err(SiltError::ExpInvalidBitwise(v.to_error())),
                    }
                }
                OpCode::DUP_N(n) => {
                    // Duplicate the top `n` values, preserving order: [a,b] -> [a,b,a,b].
                    // After each push the window shifts up by one, so grab(n) keeps
                    // yielding the next original value.
                    for _ in 0..*n {
                        let v = self.grab(ep, *n as usize).clone();
                        self.push(ep, v);
                    }
                }

                OpCode::NEGATE => {
                    match self.peek(ep) {
                        Value::Number(n) => {
                            let f = -n;
                            self.pop(ep);
                            self.push(ep, Value::Number(f))
                        }
                        Value::Integer(i) => {
                            let f = -i;
                            self.pop(ep);
                            self.push(ep, Value::Integer(f))
                        }
                        // None => Err(SiltError::EarlyEndOfFile)?,
                        c => break Err(SiltError::ExpInvalidNegation(c.to_error())),
                    }
                    // TODO  test this vs below: unsafe { *ep.ip = -*ep.ip };
                }
                OpCode::NIL => self.push(ep, Value::Nil),
                OpCode::NILS(u) => self.push_nils(ep, *u as usize),
                OpCode::TRUE => self.push(ep, Value::Bool(true)),
                OpCode::FALSE => self.push(ep, Value::Bool(false)),
                OpCode::NOT => {
                    let value = self.pop(ep);
                    self.push(ep, Value::Bool(!Self::is_truthy(&value)));
                }
                OpCode::EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(Self::is_equal(&l, &r)));
                }
                OpCode::NOT_EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(!Self::is_equal(&l, &r)));
                }
                OpCode::LESS => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(bubble!(Self::is_less(&l, &r))));
                }
                OpCode::LESS_EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(!bubble!(Self::is_greater(&l, &r))));
                }
                OpCode::GREATER => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(bubble!(Self::is_greater(&l, &r))));
                }
                OpCode::GREATER_EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(!bubble!(Self::is_less(&l, &r))));
                }
                OpCode::CONCAT => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    match (l, r) {
                        (Value::String(left), Value::String(right)) => {
                            self.push(ep, Value::String(left + &right))
                        }
                        (Value::String(left), v2) => {
                            self.push(ep, Value::String(left + &v2.to_string()))
                        }
                        (v1, Value::String(right)) => {
                            self.push(ep, Value::String(v1.to_string() + &right))
                        }
                        (v1, v2) => self.push(ep, Value::String(v1.to_string() + &v2.to_string())),
                    }
                }

                OpCode::LITERAL {
                    dest: _,
                    literal: _,
                } => {}
                OpCode::POP => {
                    self.pop(ep); //  last =
                }

                OpCode::POPS(n) => self.popn_drop(ep, *n), //TODO here's that 255 local limit again

                OpCode::CLOSE_UPVALUES(n) => {
                    self.close_n_upvalues(ep, *n);
                }

                OpCode::GOTO_IF_FALSE(offset) => {
                    let value = self.peek(ep);
                    // println!("GOTO_IF_FALSE: {}", value);
                    if !Self::is_truthy(value) {
                        frame.forward(*offset);
                    }
                }

                OpCode::POP_AND_GOTO_IF_FALSE(offset) => {
                    let value = &self.pop(ep);
                    // println!("GOTO_IF_FALSE: {}", value);
                    if !Self::is_truthy(value) {
                        frame.forward(*offset);
                    }
                }
                OpCode::GOTO_IF_TRUE(offset) => {
                    let value = self.peek(ep);
                    if Self::is_truthy(value) {
                        frame.forward(*offset);
                    }
                }
                OpCode::FORWARD(offset) => {
                    frame.forward(*offset);
                }
                OpCode::REWIND(offset) => {
                    frame.rewind(*offset);
                }

                OpCode::FOR_NUMERIC(skip) => {
                    // Stack layout: [iterator, limit, step] with step on top. The
                    // loop-continue test depends on the step's sign: ascending ends
                    // once iterator > limit, descending once iterator < limit.
                    let iterator = unsafe { &*ep.ip.sub(3) };
                    let compare = unsafe { &*ep.ip.sub(2) };
                    let step = unsafe { &*ep.ip.sub(1) };
                    let descending = match step {
                        Value::Integer(i) => *i < 0,
                        Value::Number(n) => *n < 0.0,
                        _ => false,
                    };
                    let done = if descending {
                        bubble!(Self::is_less(iterator, compare))
                    } else {
                        bubble!(Self::is_greater(iterator, compare))
                    };
                    if done {
                        frame.forward(*skip);
                    } else {
                        let it = iterator.clone();
                        self.push(ep, it);
                    }
                }
                OpCode::INCREMENT { index } => {
                    let value = frame.get_val_mut(*index);
                    let step = self.peek(ep);
                    bubble!(value.increment(step));
                }

                OpCode::FOR_GENERIC { count, exit } => {
                    // The top three stack values are the iterator triple
                    // (iterator f, state s, control). Call f(s, control).
                    let f_val = unsafe { &*ep.ip.sub(3) }.clone();
                    let s = unsafe { &*ep.ip.sub(2) }.clone();
                    let control = unsafe { &*ep.ip.sub(1) }.clone();
                    let results = match &f_val {
                        Value::NativeFunction(nf) => {
                            match bubble!(nf.f.call(self, ep.mc, &[s, control])) {
                                NativeReturn::Single(v) => vec![v],
                                NativeReturn::Multi(vals) => vals,
                            }
                        }
                        _ => {
                            break Err(SiltError::Custom(
                                "'for' iterator must be a function (custom closures not yet supported)"
                                    .into(),
                            ))
                        }
                    };
                    let first = results.first().cloned().unwrap_or(Value::Nil);
                    if matches!(first, Value::Nil) {
                        frame.forward(*exit);
                    } else {
                        // advance the control variable, then push the loop vars
                        unsafe { *ep.ip.sub(1) = first };
                        for i in 0..*count {
                            let v = results.get(i as usize).cloned().unwrap_or(Value::Nil);
                            self.push(ep, v);
                        }
                    }
                }

                OpCode::CLOSURE { constant } => {
                    let value = Self::get_chunk(&frame).get_constant(*constant);
                    devout!(" | => {}", value);
                    if let Value::Function(f) = value {
                        //     // f.upvalue_count
                        //     let mut closure =
                        //         Closure::new(f.clone(), Vec::with_capacity(f.upvalue_count as usize));
                        //     // let reserved_value = self.reserve();
                        //     if f.upvalue_count >= 0 {
                        //         let next_instruction = frame.get_next_n_codes(f.upvalue_count as usize);
                        //         for i in 0..f.upvalue_count {
                        //             devout!(" | {}", next_instruction[i as usize]);
                        //             if let OpCode::REGISTER_UPVALUE { index, neighboring } =
                        //                 next_instruction[i as usize]
                        //             {
                        //                 closure.upvalues.push(if neighboring {
                        //                     // insert at i
                        //                     self.capture_upvalue(ep, index, frame)
                        //                     // closure.upvalues.insert(
                        //                     //     i as usize,
                        //                     //     frame.function.upvalues[index as usize].clone(),
                        //                     // );
                        //                     // *slots.add(index) ?
                        //                 } else {
                        //                     frame.function.upvalues[index as usize].clone()
                        //                 });
                        //             } else {
                        //                 println!(
                        //                     "next instruction is not CLOSE_UPVALUE {}",
                        //                     next_instruction[i as usize]
                        //                 );
                        //                 unreachable!()
                        //             }
                        //         }
                        //
                        //         self.push(ep, Value::Closure(Gc::new(ep.mc, closure)));
                        //     } else {
                        //         // devout!("closure is of type {}", closure.function.function.name);
                        //         return Err(SiltError::VmRuntimeError);
                        //     }
                        //     frame.shift(f.upvalue_count as usize);
                        // }

                        bubble!(FunctionObject::push_closure(f.clone(), self, frame, ep));
                    }
                }

                OpCode::GET_UPVALUE { index } => {
                    let value = frame.function.upvalues[*index as usize]
                        .borrow()
                        .copy_value();

                    #[cfg(feature = "dev-out")]
                    {
                        frame.print_local_stack();
                        frame.function.print_upvalues();
                    }

                    devout!("GET_UPVALUE: {}", value);
                    self.push(ep, value);
                }
                OpCode::SET_UPVALUE { index } => {
                    let value = self.peek(ep); // TODO pop and set would be faster, less cloning
                    let ff = &frame.function.upvalues;
                    ff[*index as usize]
                        .borrow_mut(ep.mc)
                        .set_value(value.clone());
                    // unsafe { *upvalue.value = value };
                }

                OpCode::CALL(arity, multi,variadic) => {
                    // Resolve the true argument count. When the call spreads `...`
                    // the trailing arg expands to the caller's variadic overflow, so
                    // the compile-time count (which counts `...` as a single arg) is
                    // adjusted by the caller's own overflow.
                    let ar = if *variadic {
                        (*arity - 1) + (frame.call_arity - frame.function.get_variadic())
                    } else {
                        *arity
                    };
                    let value = self.peekn(ep, ar);
                    devout!(" | -> {}", value);
                    match value {
                        Value::Closure(c) => {
                            let cc = *c;
                            if cc.is_variadic() {
                                // Lua-style variadic adjust: the overflow args stay
                                // where they are and we copy the function slot plus
                                // the fixed params *above* them. The new frame base
                                // begins just past the variadic range so body locals
                                // address contiguously and the overflow is reachable
                                // below the base via CallFrame::get_varargs.
                                let total = ar as usize;
                                let numfixed = cc.get_variadic() as usize;
                                let snapshot = self.stack_count - total - 1;
                                // original function slot, start of the copy source
                                let func_ptr = unsafe { ep.ip.sub(total + 1) };
                                // copies land at the current top -> this is the new base
                                let frame_top = ep.ip;
                                // clone function + fixed params before pushing so we
                                // never read and write the stack at the same time
                                let copies: Vec<Value<'gc>> = (0..=numfixed)
                                    .map(|i| unsafe { (*func_ptr.add(i)).clone() })
                                    .collect();
                                for v in copies {
                                    self.push(ep, v);
                                }
                                let new_frame = CallFrame::new(cc, snapshot, ar, *multi);
                                frames.push(new_frame);
                                frame = frames.last_mut().unwrap();
                                frame.local_stack = frame_top;
                            } else {
                                let frame_top = unsafe { ep.ip.sub((ar as usize) + 1) };
                                let new_frame = CallFrame::new(
                                    cc,
                                    self.stack_count - (ar as usize) - 1,
                                    ar,
                                    *multi,
                                );
                                frames.push(new_frame);
                                frame = frames.last_mut().unwrap();
                                frame.local_stack = frame_top;
                            }
                            frame_count += 1;
                        }
                        Value::Function(_func) => {
                            // let frame_top =
                            //     unsafe { ep.ip.sub((*param_count as usize) + 1) };
                            // let new_frame = CallFrame::new(
                            //     func.clone(),
                            //     self.stack_count - (*param_count as usize) - 1,
                            // );
                            // frames.push(new_frame);
                            // frame = frames.last_mut().unwrap();

                            // frame.local_stack = frame_top;
                            // devout!("top of frame stack {}", unsafe { &*frame.local_stack });
                            // frame_count += 1;

                            // devout!("current stack count {}", frame.stack_snapshot);
                            // frame.ip = f.chunk.code.as_ptr();
                            // // frame.stack.resize(256, Value::Nil); // TODO
                            // self.push(Value::Function(f.clone())); // TODO this needs to store the function object itself somehow, RC?
                        }
                        Value::NativeFunction(_) => {
                            // get args including the function value at index 0. We do it here so don't have mutability issues with native fn
                            // TODO get a reference instead of the a-pop-olypse
                            // Use the resolved arity (`ar`) so spread `...` arguments
                            // pop the actual count, not the compile-time placeholder.
                            let mut args = self.popn(ep, ar + 1);
                            // todo!("Hi there! we need to set arity of userdata functions to include self! At least this is hirting our abstraction, we could force it but that's dangerous! Let's perhas make userdata methods Option<Self>");

                            if let Value::NativeFunction(f) = args.remove(0) {
                                let res = bubble!(f.f.call(self, ep.mc, &args));
                                match res {
                                    NativeReturn::Single(v) => self.push(ep, v),
                                    NativeReturn::Multi(vals) => {
                                        // spread the values, then adjust to the
                                        // caller's wanted count (`multi`): >1 keeps
                                        // that many (nil-padded), else just one.
                                        let n = vals.len();
                                        for v in vals {
                                            self.push(ep, v);
                                        }
                                        let want = if *multi > 1 { *multi as usize } else { 1 };
                                        if n < want {
                                            for _ in 0..(want - n) {
                                                self.push(ep, Value::Nil);
                                            }
                                        } else {
                                            for _ in 0..(n - want) {
                                                self.pop(ep);
                                            }
                                        }
                                    }
                                }
                            } else {
                                unreachable!();
                            }
                        }
                        _ => {
                            break Err(SiltError::NotCallable(format!("Value: {}", value)));
                        }
                    }
                }

                OpCode::PRINT => {
                    println!("<<<<<< {} >>>>>>>", self.pop(ep));
                }
                OpCode::META(_) => todo!(),
                OpCode::REGISTER_UPVALUE {
                    index: _,
                    neighboring: _,
                } => unreachable!(),
                OpCode::LENGTH => {
                    let value = self.pop(ep);
                    match value {
                        Value::String(s) => self.push(ep, Value::Integer(s.len() as i64)),
                        Value::Table(t) => self.push(ep, Value::Integer(t.borrow().len() as i64)),
                        _ => break Err(SiltError::ExpInvalidLength(value.to_error())),
                    }
                }
                OpCode::NEW_TABLE => {
                    self.push(ep, self.new_table(ep.mc));
                    *self.table_counter.borrow_mut() += 1;
                }
                OpCode::TABLE_INSERT { offset } => {
                    if let Err(e) = self.insert_immediate_table(ep, *offset) {
                        break Err(e);
                    }
                }
                OpCode::TABLE_BUILD(n) => {
                    if let Err(e) = self.build_table(ep, *n) {
                        break Err(e);
                    }
                }
                OpCode::TABLE_SET { depth } => {
                    let value = self.pop(ep);
                    bubble!(match self.grab(ep, *depth as usize + 1) {
                        Value::Table(_) => self.operate_table(ep, *depth, Some(value)),
                        Value::UserData(u) => {
                            let field = unsafe { ep.ip.sub(*depth as usize).replace(Value::Nil) };
                            let field_name = field.pure_string();

                            let u = &mut *(*u).borrow_mut(ep.mc);
                            let reg = &self.userdata_registry;
                            match crate::userdata::vm_integration::set_field(
                                self,
                                reg,
                                &ep.mc,
                                u,
                                &field_name,
                                value,
                            ) {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e),
                            }
                        }
                        _ => Err(SiltError::MetaMethodMissing(MetaMethod::Index)),
                    });
                }
                // OpCode::TABLE_SET_BY_CONSTANT { constant } => {
                //     let value = self.pop();
                //     let key = Self::get_chunk(&frame).get_constant(*constant);
                //     let table = self.peek_mut();
                //     if let Value::Table(t) = table {
                //         // TODO can we pre-hash this to avoid a clone?
                //         t.borrow_mut().insert(key.clone(), value);
                //     } else {
                //         return Err(SiltError::VmNonTableOperations(table.to_error()));
                //     }
                // }
                OpCode::METHOD_GET { constant } => {
                    // `obj:method(...)` — the receiver is already on top of the
                    // stack (any expression: variable, t.a.b chain, call result).
                    // Look up the method and leave [method, receiver] so the
                    // receiver is passed as the implicit `self` first argument.
                    let key = Self::get_chunk(&frame).get_constant(*constant);
                    let receiver = self.peek(ep).clone();
                    let method = match &receiver {
                        Value::Table(t) => (*t).borrow().get_value(&key),
                        // Strings dispatch methods through the `string` library
                        // (Lua's string metatable: `("x"):upper()` == string.upper("x")).
                        Value::String(_) => match self.globals.borrow().get("string") {
                            Some(Value::Table(t)) => t.borrow().get_value(&key),
                            _ => Value::Nil,
                        },
                        _ => break Err(SiltError::VmNonTableOperations(receiver.to_error())),
                    };
                    *self.peek_mut(ep) = method;
                    self.push(ep, receiver);
                }
                OpCode::TABLE_GET { depth } => {
                    let u = *depth as usize + 1;
                    let table_point = unsafe { ep.ip.sub(u) };
                    // self.table_op_index=self.stack_count-u;
                    let value = unsafe { &*table_point };

                    match value {
                        Value::Table(_) => match self.operate_table(ep, *depth, None) {
                            Ok(_) => {}
                            Err(e) => break Err(e),
                        },
                        Value::UserData(ud) => {
                            let field = unsafe { ep.ip.sub(1).replace(Value::Nil) };
                            let field_name = field.pure_string();
                            let mut mu = (*ud).borrow_mut(ep.mc);
                            let rud = mu.deref_mut();

                            match crate::userdata::vm_integration::get_field(
                                self,
                                &self.userdata_registry,
                                ep.mc,
                                rud,
                                &field_name,
                            ) {
                                Ok(value) => {
                                    self.stack_count -= u - 1;
                                    unsafe { ep.ip = ep.ip.sub(u - 1) };
                                    unsafe { table_point.replace(value) };
                                }
                                Err(e) => break Err(e),
                            }
                        }
                        _ => break Err(SiltError::VmNonTableOperations(value.to_error())),
                    }
                }
                OpCode::TABLE_GET_FROM { index: _ } => {
                    // let key = self.pop();

                    // let table = frame.get_val_mut(*index);
                    // if let Value::Table(t) = table {
                    //     let v = t.rorrow().get_value(&key);
                    //     self.push(v);
                    // } else {
                    //     return Err(SiltError::VmNonTableOperations(table.to_error()));
                    // }
                    todo!("TABLE_GET_FROM")
                }

                OpCode::TABLE_GET_BY_CONSTANT { constant } => {
                    let key = Self::get_chunk(&frame).get_constant(*constant);
                    let table = self.peek_mut(ep);
                    if let Value::Table(t) = table {
                        // let tt= t.borrow();

                        let v: Value = (*t).borrow().get_value(&key);
                        // let v:Value = t.borrow().get_value(&key);
                        self.push(ep, v);
                    } else {
                        break Err(SiltError::VmNonTableOperations(table.to_error()));
                    }
                }
            }
            frame.iterate();
            //stack
            #[cfg(feature = "dev-out")]
            {
                self.print_stack();
                println!("--------------------------------------");
            }
        };
        match results {
            Ok(o) => Ok(o),
            Err(e) => {
                let t = ErrorTuple {
                    code: e,
                    location: frame.get_loc_by_count(self.stack_count),
                };
                let stack = frames
                    .iter()
                    .map(|f| f.function.function.name.as_deref().unwrap_or("~"))
                    .collect::<Vec<&str>>()
                    .join("::");
                // let source = frame.function.function.name.clone();
                // let e = source.clone().unwrap_or("unknown".to_string());

                Err(ErrorOut {
                    errors: vec![t],
                    source: Some(stack),
                })
            }
        }
    }

    // TODO is having a default empty chunk cheaper?
    /** We're operating on the assumption a chunk is always present when using this */
    fn get_chunk<'a>(frame: &'a CallFrame<'gc>) -> &'a crate::chunk::Chunk<'gc> {
        &frame.function.function.chunk
    }

    // pub fn reset_stack(&mut self) {
    //     // TODO we probably dont even need to clear the stack, just reset the stack_top
    //     // self.stack.clear();
    //     // set to 0 index of stack
    //     ep.ip = unsafe { self.stack.as_mut_ptr() };
    // }

    fn is_truthy(v: &Value) -> bool {
        match v {
            Value::Bool(b) => *b,
            Value::Nil => false,
            _ => true,
        }
    }

    fn is_equal(l: &Value, r: &Value) -> bool {
        match (l, r) {
            (Value::Number(left), Value::Number(right)) => left == right,
            (Value::Integer(left), Value::Integer(right)) => left == right,
            (Value::Number(left), Value::Integer(right)) => *left == *right as f64,
            (Value::Integer(left), Value::Number(right)) => *left as f64 == *right,
            (Value::String(left), Value::String(right)) => left == right,
            (Value::Bool(left), Value::Bool(right)) => left == right,
            (Value::Nil, Value::Nil) => true,
            (Value::Infinity(left), Value::Infinity(right)) => left == right,
            (_, _) => false,
        }
    }

    fn is_less(l: &Value, r: &Value) -> Result<bool, SiltError> {
        Ok(match (l, r) {
            (Value::Number(left), Value::Number(right)) => left < right,
            (Value::Integer(left), Value::Integer(right)) => left < right,
            (Value::Number(left), Value::Integer(right)) => *left < *right as f64,
            (Value::Integer(left), Value::Number(right)) => (*left as f64) < (*right),
            // strings compare lexicographically by byte order (Lua's default)
            (Value::String(left), Value::String(right)) => left < right,
            (Value::Infinity(left), Value::Infinity(right)) => left != right && *left,
            (_, _) => Err(SiltError::ExpOpValueWithValue(
                l.to_error(),
                MetaMethod::Lt,
                r.to_error(),
            ))?,
        })
    }

    fn is_greater(l: &Value, r: &Value) -> Result<bool, SiltError> {
        Ok(match (l, r) {
            (Value::Number(left), Value::Number(right)) => left > right,
            (Value::Integer(left), Value::Integer(right)) => {
                // println!(" is {} > {}", left, right);
                left > right
            }
            (Value::Number(left), Value::Integer(right)) => *left > *right as f64,
            (Value::Integer(left), Value::Number(right)) => (*left as f64) > (*right),
            (Value::String(left), Value::String(right)) => left > right,
            (Value::Infinity(left), Value::Infinity(right)) => left != right && !*left,
            (_, _) => Err(SiltError::ExpOpValueWithValue(
                l.to_error(),
                MetaMethod::Gt,
                r.to_error(),
            ))?,
        })
    }

    // /** unsafe as hell, we're relying on compiler*/
    // fn read_string(&mut self, constant: u8) -> String {
    //     let value = self.get_chunk().get_constant(constant);
    //     if let Value::String(s) = value {
    //         return s.to_string();
    //     } else {
    //         unreachable!("Only strings can be identifiers")
    //     }
    // }

    // fn call(
    //     &'gc self,
    //     // ep: &mut Ephemeral<'_, 'gc>,
    //     function: &'gc Gc<Closure<'gc>>,
    //     param_count: u8,
    // ) -> CallFrame<'gc> {
    //     // let frame_top = unsafe { ep.ip.sub((param_count as usize) + 1) };
    //     let new_frame = CallFrame::new(
    //         function.clone(),
    //         self.stack_count - (param_count as usize) - 1,
    //     );
    //     new_frame
    // }

    /// call a previously stored function by it's index with optional parameters
    pub fn call_fn<T>(
        &mut self,
        mc: &Mutation<'gc>,
        name: Option<&str>,
        u: usize,
        params: T,
    ) -> LuaResult
    where
        T: for<'e> ToLuaMulti<'e>,
    {
        let source = name.map(|o| o.to_string());

        let res = match params.to_lua_multi(self, mc) {
            Ok(v) => v,
            Err(e) => {
                return Err(ErrorOut {
                    errors: vec![ErrorTuple {
                        code: e,
                        location: (0, 0),
                    }],
                    source,
                })
            }
        };

        let mut ep = Ephemeral::new(mc, self.stack.as_mut_ptr());
        self.stack_count += res.len();
        match self.external_functions.get(u) {
            Some(f) => {
                for param in res {
                    VM::push_raw(&mut ep, param);
                }

                self.run(mc, *f)
            }
            None => Err(ErrorOut {
                errors: vec![ErrorTuple {
                    code: SiltError::Unknown,
                    location: (0, 0),
                }],
                source,
            }),
        }
        // Ok(ExVal::Nil)

        // if !params.is_empty() {
        //     let mut bucket = vec![];
        //     let mut ep = Ephemeral::new(mc, self.stack.as_mut_ptr());
        //     for ev in params.into_iter() {
        //         match ev.into_value(self, mc) {
        //             Ok(v) => bucket.push(v),
        //             Err(e) => {
        //                 return Err(vec![ErrorTuple {
        //                     code: e,
        //                     location: (0, 0),
        //                 }])
        //             }
        //         }
        //     }
        //     self.stack_count += bucket.len();
        //     match self.external_functions.get(u) {
        //         Some(f) => {
        //             for param in bucket {
        //                 VM::push_raw(&mut ep, param);
        //             }
        //
        //             self.run(mc, *f)
        //         }
        //         None => Err(vec![ErrorTuple {
        //             code: SiltError::Unknown,
        //             location: (0, 0),
        //         }]),
        //     }
        // } else {
        //     match self.external_functions.get(u) {
        //         Some(f) => self.run(mc, *f),
        //         None => Err(vec![ErrorTuple {
        //             code: SiltError::Unknown,
        //             location: (0, 0),
        //         }]),
        //     }
        // }
    }

    pub(crate) fn capture_upvalue(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        index: u8,
        frame: &CallFrame<'gc>,
    ) -> Gc<'gc, RefLock<UpValue<'gc>>> {
        #[cfg(feature = "dev-out")]
        frame.print_local_stack();
        let value = unsafe { frame.local_stack.add(index as usize) };
        devout!("2capture_upvalue at index {} : {}", index, unsafe {
            &*value
        });
        // `open_upvalues` is kept sorted ASCENDING by stack address (lowest
        // first). That ordering is load-bearing: `close_n_upvalues` drains the
        // tail (the most-recently-declared locals, which sit at the highest
        // addresses). So to find or insert we scan upward and stop at the first
        // entry whose address is >= ours: equal means an upvalue already exists
        // for this slot and MUST be shared (so two closures over the same local
        // see each other's writes); greater is the insertion point.
        let mut insert_at = self.open_upvalues.len();
        for (i, up) in self.open_upvalues.iter().enumerate() {
            let loc = up.borrow().location;
            if loc == value {
                return *up;
            }
            if loc > value {
                insert_at = i;
                break;
            }
        }

        let u = Gc::new(ep.mc, RefLock::new(UpValue::new(index, value)));
        self.open_upvalues.insert(insert_at, u);

        #[cfg(feature = "dev-out")]
        self.print_upvalues();
        u

        //   self
        //     .open_upvalues
        //     .iter()
        //     // DEV originally we loop through until the pointer is not greater then the stack pointer
        //     .find(|upvalue| upvalue.index == index)
        // {
        //     Some(u) => u.clone(),
        //     None => {
        //         // let v = unsafe { stack.sub(index as usize) };
        //         let u = Rc::new(UpValue::new(index));
        //         self.open_upvalues.push(u.clone());
        //         u
        //     }
        // }

        // let mut prev = stack;
        // for _ in 0..index {
        //     prev = unsafe { prev.sub(1) };
        // }
        // unsafe { prev.read() }
    }

    #[allow(dead_code)]
    fn close_upvalue(&mut self, _value: Value) {
        devout!("close_upvalue: {}", _value);
        todo!()

        // for up in
        // self.open_upvalues
        //     .iter()
        //     .find(|up| {
        //         let mut upvalue = up.borrow_mut();
        //         if upvalue.index >= self.stack_count as u8 {
        //             false
        //         } else {
        //             true
        //         }
        //     })
        //     .unwrap()
        //     .borrow_mut()
        //     .close(value);
        // TODO
        // self.open_upvalues.retain(|up| {
        //     let mut upvalue = up.borrow_mut();
        //     if upvalue.index >= self.stack_count as u8 {
        //         upvalue.close(value);
        //         false
        //     } else {
        //         true
        //     }
        // });
    }

    /// create a new Table Value, iterating our primative table id counter
    pub fn new_table(&self, mc: &Mutation<'gc>) -> Value<'gc> {
        let t = Table::new(*self.table_counter.borrow());
        *self.table_counter.borrow_mut() += 1;
        Value::Table(Gc::new(mc, RefLock::new(t)))
    }

    /// create a new Table Value from an existing hashmap, iterates our primative table id counter
    pub fn convert_table(&mut self, mc: &Mutation<'gc>, data: &ExTable) -> InnerResult<'gc> {
        let id = *self.table_counter.borrow();
        let t = Table::wrap_map(self, mc, id, data)?;
        *self.table_counter.borrow_mut() += 1;
        Ok(Value::Table(Gc::new(mc, RefLock::new(t))))
    }

    pub fn table_from_array<A, I>(&self, mc: &Mutation<'gc>, array: I) -> Value<'gc>
    where
        I: IntoIterator<Item = A>,
        A: Into<Value<'gc>>,
    {
        let mut t = Table::new(*self.table_counter.borrow());
        *self.table_counter.borrow_mut() += 1;
        t.concat_array(array);

        Value::Table(Gc::new(mc, RefLock::new(t)))
    }

    pub fn raw_table(&self) -> Table<'gc> {
        let t = Table::new(*self.table_counter.borrow());
        *self.table_counter.borrow_mut() += 1;
        t
    }

    pub fn wrap_table(&self, mc: &Mutation<'gc>, t: Table<'gc>) -> Value<'gc> {
        Value::Table(Gc::new(mc, RefLock::new(t)))
    }

    // pub fn wrap_table(&self, t: Table) -> InnerResult<'gc> {
    //     if let Some(mc) = self.mutator_ref {
    //         return Ok(Value::Table(Gc::new(mc, RefLock::new(t))));
    //     }
    //     Err(SiltError::Unknown)
    // }

    fn build_table(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: u8) -> Result<(), SiltError> {
        let offset = n as usize + 1;
        let table_point = unsafe { ep.ip.sub(offset) };
        let table = unsafe { &*table_point };
        if let Value::Table(t) = table {
            let mut b = (*t).borrow_mut(ep.mc);
            // push in reverse
            for i in (0..n).rev() {
                let value = unsafe { ep.ip.sub(i as usize + 1).replace(Value::Nil) };
                b.raw_push(value);
            }

            self.stack_count -= offset - 1;
            ep.ip = unsafe { table_point.add(1) };

            Ok(())
        } else {
            Err(SiltError::ChunkCorrupt) // shouldn't happen unless our compiler really screwed up
        }
    }

    /** Used at table creation to simplify direct index insertion */
    fn insert_immediate_table(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        offset: u8,
    ) -> Result<(), SiltError> {
        let table = unsafe { &*ep.ip.sub(offset as usize + 3) }; // -3 because -1 for top of stack, -1 for key, -1 for value, and then offset from there
        if let Value::Table(t) = table {
            let value = self.pop(ep);
            let key = self.pop(ep);
            (*t).borrow_mut(ep.mc).set(key, value);
            Ok(())
        } else {
            Err(SiltError::ChunkCorrupt) // shouldn't happen unless our compiler really screwed up
        }
    }

    /**
     * Compares indexes on stack by depth amount, if set value not passed we act as a getter and push value at index on to stack
     * Unintentional pun
     */
    fn operate_table(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        depth: u8,
        set: Option<Value<'gc>>,
    ) -> Result<(), SiltError> {
        // let value = unsafe { ep.ip.read() };
        // let value = unsafe { ep.ip.replace(Value::Nil) };

        let u = depth as usize + 1;
        let decrease = match set {
            Some(_) => u,
            None => u - 1,
        };
        let table_point = unsafe { ep.ip.sub(u) };
        let table = unsafe { &*table_point };
        if let Value::Table(t) = table {
            let mut current = *t;
            for i in 1..=depth {
                // Keys sit on the stack in source order above the table:
                // [table, key1, key2, ... keyN] with keyN on top (ip.sub(1)).
                // Navigation must consume them left-to-right, so step i reads
                // key i at ip.sub(depth - i + 1) — NOT ip.sub(i), which would
                // walk the chain backwards (t[keyN] first → nil for depth >= 2).
                let key = unsafe { ep.ip.sub((depth - i + 1) as usize).replace(Value::Nil) };
                devout!("get from table with key: {}", key);
                if i == depth {
                    // let offset = depth as usize;
                    self.stack_count -= decrease;
                    unsafe { ep.ip = ep.ip.sub(decrease) };
                    // assert!(ep.ip == table_point);
                    match set {
                        Some(value) => {
                            current.borrow_mut(ep.mc).set(key, value);
                            unsafe { table_point.replace(Value::Nil) };
                        }
                        None => {
                            let out = current.borrow().get_value(&key);
                            unsafe { table_point.replace(out) };
                        }
                    }
                    return Ok(());
                } else {
                    let v = current.try_borrow().unwrap();
                    let check = v.getr(&key);
                    // let check = unsafe { current.try_borrow_unguarded() }.unwrap().get(&key);
                    match check {
                        Some(Value::Table(t)) => {
                            current = *t;
                        }
                        Some(v) => {
                            return Err(SiltError::VmNonTableOperations(v.to_error()));
                        }
                        None => {
                            return Err(SiltError::VmNonTableOperations(ValueTypes::Nil));
                        }
                    }
                }
            }
            Err(SiltError::VmRuntimeError)
        } else {
            Err(SiltError::VmNonTableOperations(table.to_error()))
        }

        // self.stack_count -= 1;
        // unsafe { ep.ip = ep.ip.sub(1) };
        // let v = unsafe { ep.ip.replace(Value::Nil) };
        // // TODO is there a way to read without segfaulting?
        // // We'd have to list the value to be forggoten, but is this even faster?
        // // let v = unsafe { ep.ip.read() };
        // devout!("pop: {}", v);
        // v

        // // let value = self.stack[self.stack_count - (index as usize) - 1].clone();
        // if let Value::Table(t) = value {
        //     t
        // } else {
        //     unreachable!("Only tables can be indexed")
        // }
    }

    /// Handle binary operations with UserData
    pub(crate) fn handle_userdata_binary_op(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        userdata: InnerUserData<'gc>,
        op: MetaMethod,
        right: Value<'gc>,
    ) -> Result<Value<'gc>, SiltError> {
        let u = &mut *userdata.borrow_mut(ep.mc);
        // let rud = u.deref_mut() ;
        // Try to call the metamethod
        crate::userdata::vm_integration::call_meta_method(
            self,
            &self.userdata_registry,
            &ep.mc,
            u,
            op,
            vec![right],
        )
    }

    pub fn testy<'a>(&mut self, _mc: &'a Mutation<'gc>, _name: &str) {}

    /** Register a native function on the global table  */
    // pub fn register_native_function<T, F, R>(&self, mc: &Mutation<'gc>, name: &str, function: F)
    // where
    //     R: ToLua<'gc>,
    //     F: Fn(&mut VM<'gc>, &Mutation<'gc>, T) -> ToInnerResult<'gc, R> + 'gc,
    //     T: for<'f> FromLuaMulti<'f, 'gc>,
    // {
    //     let raw = NativeFunctionRaw::new(function);
    //
    //     let f = WrappedFn { f: Rc::new(raw) };
    //
    //     self.globals
    //         .borrow_mut(mc)
    //         .insert(name.into(), Value::NativeFunction(Gc::new(mc, f)));
    // }

    // /// Register a UserData type with the VM
    // pub fn register_userdata_type<T: UserData>(&mut self) {
    //     self.userdata_registry.register::<T>();
    // }

    /// Create a UserData value
    pub fn create_userdata<T: UserData>(&mut self, mc: &Mutation<'gc>, data: T) -> Value<'gc> {
        crate::userdata::vm_integration::create_userdata(
            &mut self.userdata_registry,
            mc,
            data,
            &mut self.userdata_stack,
        )
    }

    /// Create a UserData wrapper without creating a value object
    pub fn create_userdata_raw<T: UserData>(
        &mut self,
        mc: &Mutation<'gc>,
        data: T,
    ) -> UserDataWrapper {
        crate::userdata::vm_integration::create_userdata_raw(
            &mut self.userdata_registry,
            mc,
            data,
            &mut self.userdata_stack,
        )
    }

    /// Create a UserData wrapper without creating a value object
    pub fn create_userdata_tuple<T: UserData>(
        &mut self,
        mc: &Mutation<'gc>,
        data: T,
    ) -> (Value<'gc>, WeakWrapper) {
        let (ud, weak) = crate::userdata::vm_integration::create_userdata_tuple(
            &mut self.userdata_registry,
            mc,
            data,
        );

        let ud_gc = Gc::new(mc, RefLock::new(ud));
        (Value::UserData(ud_gc), weak)
    }

    /** Load standard library functions */
    pub fn load_standard_library<'a>(&'a mut self, mc: &Mutation<'gc>) {
        // macro_rules! register_native_fn {
        //     ($name:expr, $func:expr) => {
        //         self.register_native_function::<Vec<Value>, _, _>(mc, $name, $func)
        //     };
        //
        //     ($name:expr, $func:expr, $input_type:ty) => {
        //         self.register_native_function::<$input_type, _, _>(mc, $name, $func)
        //     };
        // }

        // let v=Self::register_native_function(mc,crate::standard::clock);
        self.register_native_function(mc, "clock", crate::standard::clock);
        // self.inser( mc, "clock", v);
        // register_native_fn!("clock", crate::standard::clock, ());
        self.register_native_function(mc, "print", crate::standard::print);
        self.register_native_function(mc, "setmetatable", crate::standard::setmetatable);
        self.register_native_function(mc, "getmetatable", crate::standard::getmetatable);
        self.register_native_function(mc, "test_ent", crate::standard::test_ent);

        // base functions
        self.register_native_function(mc, "type", crate::standard::lua_type);
        self.register_native_function(mc, "tostring", crate::standard::tostring);
        self.register_native_function(mc, "tonumber", crate::standard::tonumber);
        self.register_native_function(mc, "assert", crate::standard::assert);
        self.register_native_function(mc, "error", crate::standard::error);
        self.register_native_multi_function(mc, "next", crate::standard::lua_next);
        self.register_native_multi_function(mc, "pairs", crate::standard::lua_pairs);
        self.register_native_multi_function(mc, "ipairs", crate::standard::lua_ipairs);

        let mut table = self.raw_table();
        self.register_native_function_to(mc, &mut table, "insert", crate::standard::table_insert);
        self.register_native_function_to(mc, &mut table, "remove", crate::standard::table_remove);
        let t = self.wrap_table(mc, table);
        self.globals.borrow_mut(mc).set("table", t);

        // math library
        let mut math = self.raw_table();
        self.register_native_function_to(mc, &mut math, "floor", crate::standard::math_floor);
        self.register_native_function_to(mc, &mut math, "ceil", crate::standard::math_ceil);
        self.register_native_function_to(mc, &mut math, "abs", crate::standard::math_abs);
        self.register_native_function_to(mc, &mut math, "sqrt", crate::standard::math_sqrt);
        self.register_native_function_to(mc, &mut math, "sin", crate::standard::math_sin);
        self.register_native_function_to(mc, &mut math, "cos", crate::standard::math_cos);
        self.register_native_function_to(mc, &mut math, "tan", crate::standard::math_tan);
        self.register_native_function_to(mc, &mut math, "min", crate::standard::math_min);
        self.register_native_function_to(mc, &mut math, "max", crate::standard::math_max);
        self.register_native_function_to(mc, &mut math, "random", crate::standard::math_random);
        self.register_native_function_to(
            mc,
            &mut math,
            "randomseed",
            crate::standard::math_randomseed,
        );
        math.set("pi", Value::Number(std::f64::consts::PI));
        math.set("huge", Value::Number(f64::INFINITY));
        math.set("maxinteger", Value::Integer(i64::MAX));
        math.set("mininteger", Value::Integer(i64::MIN));
        let math_t = self.wrap_table(mc, math);
        self.globals.borrow_mut(mc).set("math", math_t);

        // string library
        let mut string = self.raw_table();
        self.register_native_function_to(mc, &mut string, "len", crate::standard::string_len);
        self.register_native_function_to(mc, &mut string, "sub", crate::standard::string_sub);
        self.register_native_function_to(mc, &mut string, "upper", crate::standard::string_upper);
        self.register_native_function_to(mc, &mut string, "lower", crate::standard::string_lower);
        self.register_native_function_to(mc, &mut string, "rep", crate::standard::string_rep);
        self.register_native_function_to(
            mc,
            &mut string,
            "reverse",
            crate::standard::string_reverse,
        );
        self.register_native_function_to(mc, &mut string, "byte", crate::standard::string_byte);
        self.register_native_function_to(mc, &mut string, "char", crate::standard::string_char);
        self.register_native_function_to(mc, &mut string, "format", crate::standard::string_format);
        let string_t = self.wrap_table(mc, string);
        self.globals.borrow_mut(mc).set("string", string_t);

        // Example of closure without turbofish
        // let test = Box::new(5);
        // register_fn!("test_closure", move |_, _, _: ()| {
        //     Ok((*test).into())
        // }, ());
    }

    // pub fn register_native_function3<T, F, R>(
    //     // vm: &VM<'gc>,
    //     _function: F,
    // ) where
    //     R: ToLua<'gc> + 'gc,
    //     T: for<'a> FromLuaMulti<'gc> + 'gc,
    //     F: Fn(&mut VM<'gc>, T) -> R,
    // {
    //     // Value::NativeFunction(Gc::new(mc, f))
    // }

    pub fn register_native_function<A, F, R>(
        &mut self,
        // vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        name: &str,
        function: F,
    ) where
        A: FromLuaMulti<'gc>,
        // <T as FromLuaMulti<'gc>>::Output
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, A) -> R + 'gc,
        R: ToLua<'gc> + 'gc,
    {
        let raw = NativeFunctionRaw::new::<A, _, _>(function);

        let f = WrappedFn { f: Rc::new(raw) };
        // Value::NativeFunction(Gc::new(mc, f))
        let v = Value::NativeFunction(Gc::new(mc, f));
        // println!("add native {}, {}", name,v);
        self.globals.borrow_mut(mc).set(name, v);
    }

    /// Register a multi-return native function (raw arg slice in, `Vec<Value>` out).
    pub fn register_native_multi_function<F>(&mut self, mc: &Mutation<'gc>, name: &str, function: F)
    where
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, &[Value<'gc>]) -> Result<Vec<Value<'gc>>, SiltError>
            + 'gc,
    {
        let raw = NativeFunctionRaw::new_multi(function);
        let f = WrappedFn { f: Rc::new(raw) };
        let v = Value::NativeFunction(Gc::new(mc, f));
        self.globals.borrow_mut(mc).set(name, v);
    }
    //
    pub fn register_native_function_to<A, F, R>(
        &mut self,
        // vm: &VM<'gc>,
        mc: &Mutation<'gc>,
        table: &mut Table<'gc>,
        name: &str,
        function: F,
    ) where
        A: FromLuaMulti<'gc>,
        // <T as FromLuaMulti<'gc>>::Output
        F: Fn(&mut VM<'gc>, &Mutation<'gc>, A) -> R + 'gc,
        R: ToLua<'gc> + 'gc,
    {
        let raw = NativeFunctionRaw::new::<A, _, _>(function);

        let f = WrappedFn { f: Rc::new(raw) };
        // Value::NativeFunction(Gc::new(mc, f))
        let v = Value::NativeFunction(Gc::new(mc, f));
        table.set(name, v);
    }
    // pub fn register_native_function<T, R>(
    //     &mut self,
    //     mc: &Mutation<'gc>,
    //     name: &str,
    //     function:  fn(&mut VM<'gc>, &Mutation<'gc>, T) -> R,
    // )
    // where
    //     R: ToLua<'gc> + 'gc,
    //     T: for<'f> FromLuaMulti<'f, 'gc> + 'gc,
    // {
    //     let raw = NativeFunctionRaw::new(function);
    //
    //     let f = WrappedFn { f: Rc::new(raw) };
    //     let v=Value::NativeFunction(Gc::new(mc, f));
    //     self.globals.borrow_mut(mc).insert(name.into(), v);
    // }

    /// Clean up dropped UserData references from the userdata_stack
    // pub fn cleanup_userdata(&self) {
    //     let mut stack = self.userdata_stack.unwrap();
    //     for i in 0..stack.0.len() {
    //         if let Some(weak_wrapper) = &stack.0[i] {
    //             // Check if the UserData is still referenced in the VM
    //             if weak_wrapper.is_dropped() {
    //                 // If the original UserDataWrapper has been dropped, set to None in the stack
    //                 stack.0[i] = None;
    //             }
    //         }
    //     }
    // }

    /// Get a UserData from the userdata_stack by index
    // pub fn get_userdata_by_index(&self, index: usize) -> Option<UserDataWrapper> {
    //     let stack = self.userdata_stack.unwrap();
    //     if index < stack.0.len() {
    //         if let Some(weak_wrapper) = &stack.0[index] {
    //             return weak_wrapper.upgrade();
    //         }
    //     }
    //     None
    // }
    //
    //
    #[allow(dead_code)]
    fn print_raw_stack(&self) {
        println!("=== Stack ({}) ===", self.stack_count);
        // 0 to stack_top
        print!("[");
        for i in self.stack.iter() {
            print!("{} ", i);
        }
        print!("]");
        println!("---");
    }

    pub fn print_stack(&self) {
        println!("=== Stack ({}) ===", self.stack_count);
        print!("░");
        let mut c = 0;
        for i in self.stack.iter() {
            c += 1;
            if c > self.stack_count {
                break;
            }
            let s = format!("{:?}", i);
            if s == "nil" {
                print!("_");
            } else {
                print!("▒ {} ", i);
            }
        }
        println!("▒░");
        // println!("---");
    }
}

pub(crate) fn to_op_string(name: Option<&str>) -> Option<String> {
    name.map(|o| o.to_string())
}
