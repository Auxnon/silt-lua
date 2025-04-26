use std::{ borrow::BorrowMut, collections::HashMap, mem::take};

use gc_arena::{lock::RefLock, Arena, Collect, Gc, Mutation, Rootable};

use crate::{
    code::OpCode,
    compiler::Compiler,
    error::{ErrorTuple, SiltError, ValueTypes},
    function::{CallFrame, Closure, FunctionObject, NativeFunction, UpValue, WrappedFn},
    prelude::UserData,
    table::Table,
    userdata::{MetaMethod, UserDataRegistry},
    value::{ExVal, Value},
};

/** Convert Integer to Float, lossy for now */
macro_rules! int2f {
    ($left:ident) => {
        $left as f64
    };
}

// macro_rules! intr2f {
//     ($left:ident) => {
//         *$left as f64
//     };
// }

macro_rules! devout {
    ($($arg:tt)*) => {
        #[cfg(feature = "dev-out")]
        println!($($arg)*);
    }

}

macro_rules! str_op_str{
    ($left:ident $op:tt $right:ident $enu:ident )=>{
        (||{
            if let Ok(n1) = $left.parse::<i64>() {
                if let Ok(n2) = $right.parse::<i64>() {
                    return Ok(Value::Integer(n1 $op n2));
                }
                if let Ok(n2) = $right.parse::<f64>() {
                    return Ok(Value::Number(int2f!(n1) $op n2));
                }
            }
            if let Ok(n1) = $left.parse::<f64>() {
                if let Ok(n2) = $right.parse::<f64>() {
                    return Ok(Value::Number(n1 $op n2));
                }
            }
            return Err(SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                MetaMethod::$enu,
                ValueTypes::String,
            ));
        })()
    }
}

macro_rules! str_op_int{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        (||{
            if let Ok(n1) = $left.parse::<i64>() {
                    return Ok(Value::Integer(n1 $op $right));

            }
            if let Ok(n1) = $left.parse::<f64>() {
                    return Ok(Value::Number(n1 $op int2f!($right)));
            }
            return Err(SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                MetaMethod::$enu,
                ValueTypes::Integer,
            ));
        })()
    }
}

macro_rules! int_op_str{
    ($left:ident $op:tt $right:ident  $enu:ident)=>{
        (||{
            if let Ok(n1) = $right.parse::<i64>() {
                    return Ok(Value::Integer($left $op n1));

            }
            if let Ok(n1) = $right.parse::<f64>() {
                    return Ok(Value::Number((int2f!($left) $op n1)));
            }
            return Err(SiltError::ExpOpValueWithValue(
                ValueTypes::Integer,
                MetaMethod::$enu,
                ValueTypes::String,
            ));
        })()
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
            return Err(SiltError::ExpOpValueWithValue(
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
            return Err(SiltError::ExpOpValueWithValue(
                ValueTypes::Number,
                MetaMethod::$enu,
                ValueTypes::String,
            ))
        }
    }
}

macro_rules! binary_op_push {
    ($src:ident, $ep:ident, $frame:ident, $frames:ident, $frame_count:ident, $op:tt, $opp:tt) => {{
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
            (Value::String(left), Value::String(right)) => str_op_str!(left $op right $opp)?,
            (Value::String(left), Value::Integer(right)) => str_op_int!(left $op right $opp)?,
            (Value::Integer(left), Value::String(right)) => int_op_str!(left $op right $opp)?,
            (Value::String(left), Value::Number(right)) => str_op_num!(left $op right $opp),
            (Value::Number(left), Value::String(right)) => num_op_str!(left $op right $opp),
            (Value::Table(left), rr ) => {
                table_meta_op!($lua, $ep, $frame, $frames, $frame_count, left, $l, rr, $opp)
            },
            // TODO userdata
            (Value::UserData(left), c) => {
                table_meta_op!($lua, $ep, $frame, $frames, $frame_count, left, $l, c, $opp)
                // if let Some(f) = left.by_meta_method($lua,$opp, $r) {
                //     f(left, right)?
                // } else {
                //     op_error!(left.to_error(), $opp, right.to_error())
                // }
                // if let Ok(f) = left.by_meta_method($lua, MetaMethod::$opp, c) {
                //     f
                // } else {
                //     return Err(SiltError::VmRuntimeError); // TODO
                //     // return Err(SiltError::ExpOpValueWithValue(ValueTypes::UserData, MetaMethod::$opp, c.to_error()));
                //     // op_error!(ValueTypes::UserData $opp right.to_error())
                // }
                // Value::Nil
            }
            (ll,rr) => return Err(SiltError::ExpOpValueWithValue(ll.to_error(), MetaMethod::$opp, rr.to_error()))
        }
    };
}

macro_rules! table_meta_op {
    ($lua:ident, $ep:ident, $frame:ident, $frames:ident, $frame_count:ident, $table:ident, $left:ident, $right:ident, $opp:tt) => {{
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
                $lua.push($ep, $left);
                // $lua.push($ep,$right);
                let arity: u8 = 1;
                let val = $lua.peekn($ep, arity);
                println!(" we attempt to call {}", val);
                if let Value::Closure(c) = val {
                    const ARITY: usize = 2;
                    let frame_top = unsafe { $ep.ip.sub(ARITY) };
                    let new_frame = CallFrame::new(c.clone(), $lua.stack_count - ARITY);
                    $frames.push(new_frame);
                    $frame = $frames.last_mut().unwrap();
                    $frame.local_stack = frame_top;
                    $frame_count += 1;

                    $lua.print_stack();
                }
                // Value::Nil
                $right
            }
            Err(e) => return Err(e),
        }
    }};
}

type LuaResult = Result<ExVal, Vec<ErrorTuple>>;
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

    pub fn run(&mut self, code: &str, compiler: &mut Compiler) -> LuaResult {
        let out = self.arena.mutate_root(|mc, root| {
            match compiler.try_compile(mc, code) {
                Ok(f) => {
                    // let v: &VM=root.borrow();
                    let res: LuaResult = root.borrow_mut().run(mc, Gc::new(mc, f));

                    // let res=root..run(mc, Gc::new(mc,f));
                    res
                }
                Err(er) => Err(er),
            }

            // let obj = Gc::new(mc, object);
            // let ret=root.run(mc, obj);
            // ret
        });
        // o
        // Ok(ExVal::Nil)
        out
    }

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
}

#[derive(Collect)]
#[collect(no_drop)]
pub struct VM<'gc> {
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
    pub globals: Gc<'gc, RefLock<HashMap<String, Value<'gc>>>>, // TODO store strings as identifer usize and use that as key
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
    table_counter: usize,
    // meta_lookup: HashMap<String, MetaMethod>,
    // string_meta: Option<Gc<Table>>,
    pub userdata_registry: UserDataRegistry
}

// #[derive(Copy, Clone, Collect)]
// #[collect(no_drop)]
// struct Object<'gc, T> {
//     value: T,
// }

// type ObjectPtr<'gc, T> = Gc<'gc, RefLock<Object<'gc, T>>>;
type ObjectPtr<'gc, T> = Gc<'gc, RefLock<T>>;
type Body<'gc> = Gc<'gc, RefLock<FunctionObject<'gc>>>;

pub(crate) struct Ephemeral<'a, 'T> {
    pub(crate) ip: *mut Value<'T>,
    pub(crate) mc: &'a Mutation<'T>,
}

impl<'a, 'T> Ephemeral<'a, 'T> {
    pub fn new(mc: &'a Mutation<'T>, ip: *mut Value<'T>) -> Self {
        Ephemeral { mc, ip }
    }
}

fn wrap<'gc, T: Collect>(mc: &Mutation<'gc>, value: T) -> ObjectPtr<'gc, T> {
    Gc::new(mc, RefLock::new(value))
}

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
            body: Gc::new(mc, FunctionObject::new(None, false)),
            // dummy_frame: CallFrame::new(Rc::new(FunctionObject::new(None, false))),
            // frames: vec![],
            // ip: 0 as *const OpCode,
            // stack, //: unsafe { *stack },
            stack_count: 0,
            stack,
            // stack_top,
            globals: Gc::new(mc, RefLock::new(HashMap::new())), //Gc::new(mc, gtable),
            open_upvalues: vec![],
            table_counter: 1,
            userdata_registry: UserDataRegistry::new(),
        }
    }

    pub(crate) fn push(&mut self, ep: &mut Ephemeral<'_, 'gc>, value: Value<'gc>) {
        devout!(" | push: {}", value);
        unsafe { ep.ip.write(value) };
        ep.ip = unsafe { ep.ip.add(1) };
        self.stack_count += 1;
    }

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
                .for_each(|mut up| {
                    let mut upvalue = up.borrow_mut(ep.mc);
                    upvalue.close_around(unsafe { ep.ip.replace(Value::Nil) });
                });
            unsafe { ep.ip = ep.ip.sub(n as usize) };
        } else {
            let upvalue = self.open_upvalues.pop().unwrap();

            upvalue.borrow_mut(ep.mc).close_around(self.pop(ep));
        }
    }

    fn close_upvalues_by_return(&mut self, last: *mut Value<'gc>) {
        // devout!("value: {}", unsafe { &*last });
        #[cfg(feature = "dev-out")]
        self.print_upvalues();
        for upvalue in self.open_upvalues.iter().rev() {
            let mut up = unsafe { upvalue.as_ptr().read() }; // TODO more bad practice
                                                             // let upv = unsafe { &*up.get_location() };
                                                             // let vv = unsafe { &*last };
                                                             // let b = up.get_location() < last;
                                                             // println!("upvalue {} less than {} is {} ", upv, vv, b);
            if up.get_location() < last {
                break;
            }
            up.close();
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
        values.reverse();
        values
    }

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

        let v0 = take(&mut self.stack[self.stack_count - 1]);

        // for i in self.stack.iter_mut().enumerate() {
        //     *i = Value::Nil;
        // }

        v0
    }

    /** Dangerous!  */
    fn read_top(&self, ep: &mut Ephemeral<'_, 'gc>) -> Value<'gc> {
        // match self.peek(0) {
        //     Some(v) => v.clone(),
        //     None => Value::Nil,
        // }

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
    fn peek_mut(&mut self, ep: &mut Ephemeral<'_, 'gc>) -> &mut Value<'gc> {
        unsafe { &mut *ep.ip.sub(1) }
    }

    fn grab(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: usize) -> &Value<'gc> {
        unsafe { &*ep.ip.sub(n) }
    }

    fn grab_mut(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: usize) -> &mut Value<'gc> {
        unsafe { &mut *ep.ip.sub(n) }
    }

    /** Look down N amount of stack and return immutable reference */
    fn peekn(&self, ep: &mut Ephemeral<'_, 'gc>, n: u8) -> &Value<'gc> {
        // unsafe { *ep.ip.sub(n as usize) }
        // &self.stack[self.stack.len() - n as usize]
        unsafe { &*ep.ip.sub((n as usize) + 1) }
    }

    // pub fn evaluate(&mut self, source: &str) -> FunctionObject<'lua> {
    //     self.compiler.compile(source.to_owned())
    // }

    pub fn run(
        &mut self,
        mc: &Mutation<'gc>,
        object: Gc<'gc, FunctionObject<'gc>>,
    ) -> Result<ExVal, Vec<ErrorTuple>> {
        // let object = self.compiler.compile(source.to_owned());
        // object.chunk.print_chunk(name)
        let out = match self.execute(mc, object.into()) {
            Ok(v) => Ok(v),
            Err(e) => Err(vec![ErrorTuple {
                code: e,
                location: (0, 0),
            }]),
        };

        // Ok(ExVal::Nil)
        out
    }

    pub fn execute(
        &mut self,
        mc: &Mutation<'gc>,
        object: Gc<'gc, FunctionObject<'gc>>,
    ) -> Result<ExVal, SiltError> {
        // TODO param is a reference of &'a
        // self.ip = object.chunk.code.as_ptr();
        // frame.ip = object.chunk.code.as_ptr();
        // frame.slots = self.stack ???
        // let rstack = self.stack.as_ptr();
        #[cfg(feature = "dev-out")]
        object.chunk.print_chunk(None);
        let mut ep = Ephemeral::new(mc, self.stack.as_mut_ptr() as *mut Value);
        self.body = object;
        // *root = new_body(mc, object.clone());
        let closure = Gc::new(mc, Closure::new(object, vec![]));

        let mut frame = CallFrame::new(closure, 0);
        frame.ip = object.chunk.code.as_ptr();
        frame.local_stack = ep.ip;
        // frame.stack.resize(256, Value::Nil); // TODO
        self.push(&mut ep, Value::Function(object)); // TODO this needs to store the function object itself somehow, RC?
        let frames = vec![frame];
        // self.body = object;
        let res = self.process(&mut ep, frames);
        // self.body = Rc::new(FunctionObject::new(None, false));

        res
    }

    fn process<'frame>(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        mut frames: Vec<CallFrame<'gc>>,
    ) -> Result<ExVal, SiltError> {
        let mut last = Value::Nil; // TODO temporary for testing
                                   // let stack_pointer = self.stack.as_mut_ptr();
                                   // let mut dummy_frame = CallFrame::new(Rc::new(FunctionObject::new(None, false)), 0);
        let mut frame = frames.last_mut().unwrap();
        let mut frame_count = 1;
        // body.chunk.print_chunk(None);
        loop {
            let instruction = frame.current_instruction();

            // devout!("ip: {:p} | {}", self.ip, instruction);
            devout!(" | {}", instruction);

            // TODO how much faster would it be to order these ops in order of usage, does match hash? probably.
            match instruction {
                OpCode::RETURN => {
                    frame_count -= 1;
                    if frame_count <= 0 {
                        if self.stack_count <= 1 {
                            return Ok(ExVal::Nil);
                        }
                        let out: ExVal = self.safe_pop().into();
                        return Ok(out);
                    }
                    let res = self.pop(ep);
                    ep.ip = frame.local_stack;
                    self.close_upvalues_by_return(ep.ip);
                    devout!("stack top {}", unsafe { &*ep.ip });
                    self.stack_count = frame.stack_snapshot;
                    frames.pop();
                    frame = frames.last_mut().unwrap();
                    devout!("next instruction {}", frame.current_instruction());
                    #[cfg(feature = "dev-out")]
                    self.print_stack();
                    self.push(ep, res);

                    // println!("<< {}", self.pop());
                    // match self.pop() {
                    //     Some(v) => return Ok(v),
                    //     None => return Ok(last),
                    // }
                }
                OpCode::CONSTANT { constant } => {
                    let value = Self::get_chunk(&frame).get_constant(*constant);
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
                        // DEV inline pop due to self lifetime nonsense
                        self.stack_count -= 1;
                        unsafe { ep.ip = ep.ip.sub(1) };
                        let v = unsafe { ep.ip.read() };

                        // let v = self.pop();
                        self.globals.borrow_mut(ep.mc).insert(s.to_string(), v);
                    } else {
                        return Err(SiltError::VmCorruptConstant);
                    }
                }

                // TODO does this need to exist?
                OpCode::SET_GLOBAL { constant } => {
                    let value = self.body.chunk.get_constant(*constant);
                    if let Value::String(s) = value {
                        let v = self.duplicate(ep);
                        // TODO we could take, expr statements send pop, this is a hack of sorts, ideally the compiler only sends a pop for nonassigment
                        // alternatively we can peek the value, that might be better to prevent side effects
                        // do we want expressions to evaluate to a value? probably? is this is ideal for implicit returns?

                        // if let Some(_) = self.globals.get(&**s) {
                        //     self.globals.insert(s.to_string(), v);
                        // } else {
                        //     self.globals.insert(s.to_string(), v);
                        // }
                        self.globals.borrow_mut(ep.mc).insert(s.to_string(), v);
                    } else {
                        devout!("SET_GLOBAL: {}", value);
                        #[cfg(feature = "dev-out")]
                        self.body.chunk.print_constants();
                        return Err(SiltError::VmCorruptConstant);
                    }
                }
                OpCode::GET_GLOBAL { constant } => {
                    let value = Self::get_chunk(&frame).get_constant(*constant);
                    if let Value::String(s) = value {
                        if let Some(v) = self.globals.borrow_mut(ep.mc).get(&**s) {
                            self.push(ep, v.clone());
                        } else {
                            self.push(ep, Value::Nil);
                        }
                    } else {
                        return Err(SiltError::VmCorruptConstant);
                    }
                }
                OpCode::SET_LOCAL { index } => {
                    let value = self.duplicate(ep);
                    // frame.stack[*index as usize] = value;
                    frame.set_val(*index, value)
                }
                OpCode::GET_LOCAL { index } => {
                    self.push(ep, frame.get_val(*index).clone());
                    // self.push(frame.stack[*index as usize].clone());
                    // TODO ew cloning, is our cloning optimized yet?
                    // TODO also we should convert from stack to register based so we can use the index as a reference instead
                }

                OpCode::DEFINE_LOCAL { constant } => todo!(),
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
                                left,
                                rr,
                                Div
                            );
                            self.push(ep, v);
                        }
                        (l, r) => {
                            return Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                MetaMethod::Div,
                                r.to_error(),
                            ))
                        }
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
                        c => Err(SiltError::ExpInvalidNegation(c.to_error()))?,
                    }
                    // TODO  test this vs below: unsafe { *ep.ip = -*ep.ip };
                }
                OpCode::NIL => self.push(ep, Value::Nil),
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
                    self.push(ep, Value::Bool(Self::is_less(&l, &r)?));
                }
                OpCode::LESS_EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(!Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER_EQUAL => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    self.push(ep, Value::Bool(!Self::is_less(&l, &r)?));
                }
                OpCode::CONCAT => {
                    let r = self.pop(ep);
                    let l = self.pop(ep);
                    match (l, r) {
                        (Value::String(left), Value::String(right)) => {
                            self.push(ep, Value::String(Box::new(*left + &right)))
                        }
                        (Value::String(left), v2) => {
                            self.push(ep, Value::String(Box::new(*left + &v2.to_string())))
                        }
                        (v1, Value::String(right)) => {
                            self.push(ep, Value::String(Box::new(v1.to_string() + &right)))
                        }
                        (v1, v2) => self.push(
                            ep,
                            Value::String(Box::new(v1.to_string() + &v2.to_string())),
                        ),
                    }
                }

                OpCode::LITERAL { dest, literal } => {}
                OpCode::POP => {
                    last = self.pop(ep);
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
                    // for needs it's own version of the stack for upvalues?
                    // compare, if greater then we skip, if less or equal we continue and then increment AFTER block
                    // let increment = self.grab(1);
                    let iterator = unsafe { &mut *ep.ip.sub(3) };
                    let compare = self.grab(ep, 2);
                    if Self::is_greater(iterator, compare)? {
                        frame.forward(*skip);
                    } else {
                        self.push(ep, iterator.clone())
                    }
                    // self.push(iterator.clone());
                    // if iterator > compare {
                    //     frame.forward(*skip);
                    // }
                }
                OpCode::INCREMENT { index } => {
                    let value = frame.get_val_mut(*index);
                    let step = self.peek(ep);
                    value.increment(step)?;
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

                        FunctionObject::push_closure(f.clone(), self, frame, ep)?;
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

                OpCode::CALL(arity) => {
                    let value = self.peekn(ep, *arity);
                    devout!(" | -> {}", value);
                    match value {
                        Value::Closure(c) => {
                            // TODO this logic is identical to function, but to make this a function causes some lifetime issues. A macro would work but we're already a little macro heavy aren't we?
                            // let frame_top = unsafe { ep.ip.sub((*param_count as usize) + 1) };
                            // let new_frame = CallFrame::new(
                            //     c.clone(),
                            //     self.stack_count - (*param_count as usize) - 1,
                            // );
                            // frames.push(new_frame);
                            // frame = frames.last_mut().unwrap();
                            //
                            // frame.local_stack = frame_top;

                            // let frame_top = unsafe { ep.ip.sub((*arity as usize) + 1) };
                            // let new_frame =
                            //     CallFrame::new(c.clone(), self.stack_count - (*arity as usize) - 1);
                            // frames.push(new_frame);
                            // frame = frames.last_mut().unwrap();
                            // frame.local_stack = frame_top;
                            let arity = *arity as usize;

                            let frame_top = unsafe { ep.ip.sub(arity + 1) };
                            let new_frame = CallFrame::new(c.clone(), self.stack_count - arity - 1);
                            frames.push(new_frame);
                            frame = frames.last_mut().unwrap();
                            frame.local_stack = frame_top;
                            frame_count += 1;
                            devout!("top of frame stack {}", unsafe { &*frame.local_stack });
                        }
                        Value::Function(func) => {
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
                            let mut args = self.popn(ep, *arity + 1);
                            if let Value::NativeFunction(f) = args.remove(0) {
                                let res = (f.f)(self, ep.mc, args);
                                // self.popn_drop(*param_count);
                                self.push(ep, res);
                            } else {
                                unreachable!();
                            }
                        }
                        _ => {
                            return Err(SiltError::NotCallable(format!("Value: {}", value)));
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
                        _ => Err(SiltError::ExpInvalidLength(value.to_error()))?,
                    }
                }
                OpCode::NEW_TABLE => {
                    self.push(ep, self.new_table(ep.mc));
                    self.table_counter += 1;
                }
                OpCode::TABLE_INSERT { offset } => {
                    self.insert_immediate_table(ep, *offset)?;
                }
                OpCode::TABLE_BUILD(n) => {
                    self.build_table(ep, *n)?;
                }
                OpCode::TABLE_SET { depth } => {
                    let value = self.pop(ep);
                    match value {
                        Value::Table(_) => self.operate_table(ep, *depth, Some(value)),
                        Value::UserData(u) => {
                            let field = unsafe { ep.ip.sub(*depth as usize).replace(Value::Nil) };
                            let field_name = field.to_string();
                            
                            // Use the new VM integration helpers
                            match crate::userdata::vm_integration::set_field(self, &u, &field_name, set.unwrap()) {
                                Ok(_) => Ok(()),
                                Err(e) => Err(e),
                            }
                        }
                        _ => Err(SiltError::MetaMethodMissing(MetaMethod::Index)),
                    }?;
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
                OpCode::TABLE_GET { depth } => {
                    match self.operate_table(ep, *depth, None) {
                        Ok(_) => {},
                        Err(e) => {
                            // If table operation failed, check if we're dealing with UserData
                            let u = *depth as usize + 1;
                            let table_point = unsafe { ep.ip.sub(u) };
                            let value = unsafe { &*table_point };
                            
                            if let Value::UserData(u) = value {
                                let field = unsafe { ep.ip.sub(1).replace(Value::Nil) };
                                let field_name = field.to_string();
                                
                                // Use the VM integration helpers
                                match userdata::vm_integration::get_field(self, &u, &field_name) {
                                    Ok(value) => {
                                        self.stack_count -= u - 1;
                                        unsafe { ep.ip = ep.ip.sub(u - 1) };
                                        unsafe { table_point.replace(value) };
                                    },
                                    Err(_) => return Err(e),
                                }
                            } else {
                                return Err(e);
                            }
                        }
                    }
                }
                OpCode::TABLE_GET_FROM { index } => {
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
                        return Err(SiltError::VmNonTableOperations(table.to_error()));
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

    fn call(
        &'gc self,
        ep: &mut Ephemeral<'_, 'gc>,
        function: &'gc Gc<Closure<'gc>>,
        param_count: u8,
    ) -> CallFrame {
        let frame_top = unsafe { ep.ip.sub((param_count as usize) + 1) };
        let new_frame = CallFrame::new(
            function.clone(),
            self.stack_count - (param_count as usize) - 1,
        );
        new_frame
    }

    pub(crate) fn capture_upvalue(
        &mut self,
        ep: &mut Ephemeral<'_, 'gc>,
        index: u8,
        frame: &CallFrame<'gc>,
    ) -> Gc<'gc, RefLock<UpValue<'gc>>> {
        //, stack: *mut Value,
        //stack
        // self.print_stack();
        #[cfg(feature = "dev-out")]
        frame.print_local_stack();
        let value = unsafe { frame.local_stack.add(index as usize) };
        devout!("2capture_upvalue at index {} : {}", index, unsafe {
            &*value
        });
        let mut ind = None;
        for (i, up) in self.open_upvalues.iter().enumerate() {
            let u = *up;
            let upvalue = u.borrow();
            if upvalue.location == value {
                return u.clone();
            }

            if upvalue.location < value {
                break;
            }
            ind = Some(i);
        }

        let u = Gc::new(ep.mc, RefLock::new(UpValue::new(index, value)));

        match ind {
            Some(i) => self.open_upvalues.insert(i, u.clone()),
            None => self.open_upvalues.push(u.clone()),
        }

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

    fn close_upvalue(&mut self, value: Value) {
        devout!("close_upvalue: {}", value);

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

    fn new_table(&self, mc: &Mutation<'gc>) -> Value<'gc> {
        let t = Table::new(self.table_counter);
        Value::Table(Gc::new(mc, RefLock::new(t)))
    }

    fn build_table(&mut self, ep: &mut Ephemeral<'_, 'gc>, n: u8) -> Result<(), SiltError> {
        let offset = n as usize + 1;
        let table_point = unsafe { ep.ip.sub(offset) };
        let table = unsafe { &*table_point };
        if let Value::Table(t) = table {
            let mut b = (*t).borrow_mut(ep.mc);
            // push in reverse
            for i in (0..n).rev() {
                let value = unsafe { ep.ip.sub(i as usize + 1).replace(Value::Nil) };
                b.push(value);
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
            (*t).borrow_mut(ep.mc).insert(key, value);
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
                let key = unsafe { ep.ip.sub(i as usize).replace(Value::Nil) };
                devout!("get from table with key: {}", key);
                if i == depth {
                    // let offset = depth as usize;
                    self.stack_count -= decrease;
                    unsafe { ep.ip = ep.ip.sub(decrease) };
                    // assert!(ep.ip == table_point);
                    match set {
                        Some(value) => {
                            current.borrow_mut(ep.mc).insert(key, value);
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
                    let check = v.get(&key);
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

    pub(crate) fn userdata_op(&mut self, ud: &mut UserData, op: MetaMethod, val: Value) {
        match self
            .userdata_methods_lookup
            .borrow_mut()
            .0
            .get_mut(&ud.type_id)
        {
            Some(f) => f(self, ud, val),
            None => Err(SiltError::MetaMethodNotCallable(op)),
        }

        // userdata_fields_lookup: Gc::new(mc, FieldsLookup::new()),
        // userdata_methods_lookup: Gc::new(mc, MethodsLookup::new()),
    }

    /** Register a native function on the global table  */
    pub fn register_native_function(
        &mut self,
        mc: &Mutation<'gc>,
        name: &str,
        function: NativeFunction<'gc>,
    ) {
        // let fn_obj = NativeObject::new(name, function);
        // let g= Gc::new(mc, function);
        let f = WrappedFn { f: function };

        self.globals
            .borrow_mut(mc)
            .insert(name.to_string(), Value::NativeFunction(Gc::new(mc, f)));
    }

    /** Load standard library functions */
    pub fn load_standard_library(&mut self, mc: &Mutation<'gc>) {
        self.register_native_function(mc, "clock", crate::standard::clock);
        self.register_native_function(mc, "print", crate::standard::print);
        self.register_native_function(mc, "setmetatable", crate::standard::setmetatable);
        self.register_native_function(mc, "getmetatable", crate::standard::getmetatable);
    }

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
