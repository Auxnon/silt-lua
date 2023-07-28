use std::{fmt::format, mem::take, rc::Rc};

use crate::{
    chunk::Chunk,
    code::OpCode,
    error::{ErrorTypes, SiltError},
    function::{CallFrame, FunctionObject},
    token::Operator,
    value::{Reference, Value},
};

/** Convert Integer to Float, lossy for now */
macro_rules! int2f {
    ($left:ident) => {
        $left as f64
    };
}

macro_rules! intr2f {
    ($left:ident) => {
        *$left as f64
    };
}
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
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::String,
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
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::Integer,
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
                ErrorTypes::Integer,
                Operator::$enu,
                ErrorTypes::String,
            ));
        })()
    }
}

macro_rules! op_error {
    ($left:ident $op:ident $right:ident ) => {{
        return Err(SiltError::ExpOpValueWithValue($left, Operator::$op, $right));
    }};
}

macro_rules! str_op_num{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        if let Ok(n1) = $left.parse::<f64>() {
            Value::Number(n1 $op $right)
        }else {
            return Err(SiltError::ExpOpValueWithValue(
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::String,
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
                ErrorTypes::Number,
                Operator::$enu,
                ErrorTypes::String,
            ))
        }
    }
}

macro_rules! binary_op {
    ($src:ident, $op:tt, $opp:tt) => {
        {

            // TODO test speed of this vs 1 pop and a mutate
            let r = $src.pop();
            let l = $src.pop();

            $src.push(match (l, r) {
                (Value::Number(left), Value::Number(right)) => (Value::Number(left $op right)),
                (Value::Integer(left), Value::Integer(right)) => (Value::Integer(left $op right)),
                (Value::Number(left), Value::Integer(right)) => (Value::Number(left $op right as f64)),
                (Value::Integer(left), Value::Number(right)) =>(Value::Number(left as f64 $op right)),
                (Value::String(left), Value::String(right)) => str_op_str!(left $op right $opp)?,
                (Value::String(left), Value::Integer(right)) => str_op_int!(left $op right $opp)?,
                (Value::Integer(left), Value::String(right)) => int_op_str!(left $op right $opp)?,
                (Value::String(left), Value::Number(right)) => str_op_num!(left $op right $opp),
                (Value::Number(left), Value::String(right)) => num_op_str!(left $op right $opp),
                (ll,rr) => return Err(SiltError::ExpOpValueWithValue(ll.to_error(), Operator::$opp, rr.to_error()))
            });
        }
    };
}
pub struct VM {
    body: Rc<FunctionObject>,
    // frames: Vec<CallFrame>,
    // dummy_frame: CallFrame,
    /** Instruction to be run at start of loop  */
    // ip: *const OpCode, // TODO usize vs *const OpCode, will rust optimize the same?
    // stack: Vec<Value>, // TODO fixed size array vs Vec
    stack: [Value; 256],
    stack_top: *mut Value,
    stack_count: usize,
    /** Next empty location */
    // stack_top: *mut Value,
    globals: hashbrown::HashMap<String, Value>, // TODO store strings as identifer usize and use that as key
                                                // references: Vec<Reference>,
                                                // obj
                                                // TODO should we store all strings in their own table/array for better equality checks? is this cheaper?
}

impl<'a> VM {
    pub fn new() -> Self {
        // TODO try the hard way
        // force array to be 256 Values
        // let stack = unsafe {
        //     std::alloc::alloc(std::alloc::Layout::array::<Value>(256).unwrap()) as *mut [Value; 256]
        // };
        // let stack: [Value; 256] = [const { Value::Nil }; 256];
        let mut stack = [(); 256].map(|_| Value::default());
        let stack_top = stack.as_mut_ptr() as *mut Value;
        // let stack = vec![];
        // let stack_top = stack.as_ptr() as *mut Value;
        Self {
            body: Rc::new(FunctionObject::new(None, false)),
            // dummy_frame: CallFrame::new(Rc::new(FunctionObject::new(None, false))),
            // frames: vec![],
            // ip: 0 as *const OpCode,
            // stack, //: unsafe { *stack },
            stack_count: 0,
            stack,
            stack_top,
            globals: hashbrown::HashMap::new(),
        }
    }

    fn push(&mut self, value: Value) {
        devout!("push: {}", value);
        unsafe { self.stack_top.write(value) };
        self.stack_top = unsafe { self.stack_top.add(1) };
        self.stack_count += 1;
    }

    /** pop N number of values from stack */
    fn popn(&mut self, n: u8) {
        unsafe { self.stack_top = self.stack_top.sub(n as usize) };
        self.stack_count -= n as usize;
    }

    /** pop and return top of stack */
    fn pop(&mut self) -> Value {
        self.stack_count -= 1;
        unsafe { self.stack_top = self.stack_top.sub(1) };
        let v = unsafe { self.stack_top.read() };
        devout!("pop: {}", v);
        v
    }

    /** Dangerous!  */
    fn read_top(&self) -> Value {
        // match self.peek(0) {
        //     Some(v) => v.clone(),
        //     None => Value::Nil,
        // }

        unsafe { self.stack_top.sub(1).read() }
    }

    /** Safer but clones! */
    fn duplicate(&self) -> Value {
        self.read_top().clone()
    }

    /** Look and get immutable reference to top of stack */
    fn peek(&self) -> &Value {
        // self.stack.last()
        unsafe { &*self.stack_top.sub(1) }
    }

    /** Look down N amount of stack and return immutable reference */
    fn peekn(&self, n: u8) -> &Value {
        // unsafe { *self.stack_top.sub(n as usize) }
        // &self.stack[self.stack.len() - n as usize]
        unsafe { &*self.stack_top.sub((n as usize) + 1) }
    }

    pub fn interpret(&mut self, object: Rc<FunctionObject>) -> Result<Value, SiltError> {
        // TODO param is a reference of &'a
        // self.ip = object.chunk.code.as_ptr();
        // frame.ip = object.chunk.code.as_ptr();
        // frame.slots = self.stack ???
        // let rstack = self.stack.as_ptr();
        self.stack_top = self.stack.as_mut_ptr() as *mut Value;
        self.body = object.clone();
        let mut frame = CallFrame::new(self.body.clone(), 0);
        frame.ip = object.chunk.code.as_ptr();
        frame.local_stack = self.stack_top;
        // frame.stack.resize(256, Value::Nil); // TODO
        self.push(Value::Function(object)); // TODO this needs to store the function object itself somehow, RC?
        let frames = vec![frame];
        // self.body = object;
        let res = self.run(frames);
        // self.frames.clear();
        res
    }

    fn run(&mut self, mut frames: Vec<CallFrame>) -> Result<Value, SiltError> {
        let mut last = Value::Nil; // TODO temporary for testing
                                   // let stack_pointer = self.stack.as_mut_ptr();
                                   // let mut dummy_frame = CallFrame::new(Rc::new(FunctionObject::new(None, false)), 0);
        let mut frame = frames.last_mut().unwrap();
        let mut frame_count = 1;
        loop {
            let instruction = frame.current_instruction();

            // devout!("ip: {:p} | {}", self.ip, instruction);
            devout!(" | {}", instruction);

            // TODO how much faster would it be to order these ops in order of usage, does match hash? probably.
            match instruction {
                OpCode::RETURN => {
                    let res = self.pop();
                    frame_count -= 1;
                    if frame_count <= 0 {
                        return Ok(res);
                    }
                    self.stack_top = frame.local_stack;
                    devout!("stack top {}", unsafe { &*self.stack_top });
                    self.stack_count = frame.stack_snapshot;
                    frames.pop();
                    frame = frames.last_mut().unwrap();
                    #[cfg(feature = "dev-out")]
                    self.print_stack(frame);
                    self.push(res);

                    // println!("<< {}", self.pop());
                    // match self.pop() {
                    //     Some(v) => return Ok(v),
                    //     None => return Ok(last),
                    // }
                }
                OpCode::CONSTANT { constant } => {
                    let value = Self::get_chunk(&frame).get_constant(*constant);
                    devout!("CONSTANT value {}", value);
                    self.push(value.clone());
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
                        unsafe { self.stack_top = self.stack_top.sub(1) };
                        let v = unsafe { self.stack_top.read() };

                        // let v = self.pop();
                        self.globals.insert(s.to_string(), v);
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                // TODO does this need to exist?
                OpCode::SET_GLOBAL { constant } => {
                    let value = self.body.chunk.get_constant(*constant);
                    if let Value::String(s) = value {
                        let v = self.duplicate();
                        // TODO we could take, expr statements send pop, this is a hack of sorts, ideally the compiler only sends a pop for nonassigment
                        // alternatively we can peek the value, that might be better to prevent side effects
                        // do we want expressions to evaluate to a value? probably? is this is ideal for implicit returns?

                        // if let Some(_) = self.globals.get(&**s) {
                        //     self.globals.insert(s.to_string(), v);
                        // } else {
                        //     self.globals.insert(s.to_string(), v);
                        // }
                        self.globals.insert(s.to_string(), v);
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                OpCode::GET_GLOBAL { constant } => {
                    let value = Self::get_chunk(&frame).get_constant(*constant);
                    if let Value::String(s) = value {
                        if let Some(v) = self.globals.get(&**s) {
                            self.push(v.clone());
                        } else {
                            self.push(Value::Nil);
                        }
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                OpCode::SET_LOCAL { index } => {
                    let value = self.duplicate();
                    // frame.stack[*index as usize] = value;
                    frame.set_val(*index, value)
                }
                OpCode::GET_LOCAL { index } => {
                    self.push(frame.get_val(*index).clone());
                    // self.push(frame.stack[*index as usize].clone());
                    // TODO ew cloning, is our cloning optimized yet?
                    // TODO also we should convert from stack to register based so we can use the index as a reference instead
                }
                OpCode::DEFINE_LOCAL { constant } => todo!(),
                OpCode::ADD => binary_op!(self, +, Add),
                OpCode::SUB => binary_op!(self,-, Sub),
                OpCode::MULTIPLY => binary_op!(self,*, Multiply),
                OpCode::DIVIDE => {
                    let right = self.pop();
                    let left = self.pop();

                    match (left, right) {
                        (Value::Number(left), Value::Number(right)) => {
                            self.push(Value::Number(left / right))
                        }
                        (Value::Integer(left), Value::Integer(right)) => {
                            self.push(Value::Number(left as f64 / right as f64))
                        }
                        (Value::Number(left), Value::Integer(right)) => {
                            self.push(Value::Number(left / right as f64))
                        }
                        (Value::Integer(left), Value::Number(right)) => {
                            self.push(Value::Number(left as f64 / right))
                        }
                        (l, r) => {
                            return Err(SiltError::ExpOpValueWithValue(
                                l.to_error(),
                                Operator::Divide,
                                r.to_error(),
                            ))
                        }
                    }
                }
                OpCode::NEGATE => {
                    match self.peek() {
                        (Value::Number(n)) => {
                            let f = -n;
                            self.pop();
                            self.push(Value::Number(f))
                        }
                        (Value::Integer(i)) => {
                            let f = -i;
                            self.pop();
                            self.push(Value::Integer(f))
                        }
                        // None => Err(SiltError::EarlyEndOfFile)?,
                        (c) => Err(SiltError::ExpInvalidNegation(c.to_error()))?,
                    }
                    // TODO  test this vs below: unsafe { *self.stack_top = -*self.stack_top };
                }
                OpCode::NIL => self.push(Value::Nil),
                OpCode::TRUE => self.push(Value::Bool(true)),
                OpCode::FALSE => self.push(Value::Bool(false)),
                OpCode::NOT => {
                    let value = self.pop();
                    self.push(Value::Bool(!Self::is_truthy(&value)));
                }
                OpCode::EQUAL => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(Self::is_equal(&l, &r)));
                }
                OpCode::NOT_EQUAL => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(!Self::is_equal(&l, &r)));
                }
                OpCode::LESS => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(Self::is_less(&l, &r)?));
                }
                OpCode::LESS_EQUAL => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(!Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER_EQUAL => {
                    let r = self.pop();
                    let l = self.pop();
                    self.push(Value::Bool(!Self::is_less(&l, &r)?));
                }
                OpCode::CONCAT => {
                    let r = self.pop();
                    let l = self.pop();
                    match (l, r) {
                        (Value::String(left), Value::String(right)) => {
                            self.push(Value::String(Box::new(*left + &right)))
                        }
                        (Value::String(left), v2) => {
                            self.push(Value::String(Box::new(*left + &v2.to_string())))
                        }
                        (v1, Value::String(right)) => {
                            self.push(Value::String(Box::new(v1.to_string() + &right)))
                        }
                        (v1, v2) => {
                            self.push(Value::String(Box::new(v1.to_string() + &v2.to_string())))
                        }
                    }
                }

                OpCode::LITERAL { dest, literal } => {}
                OpCode::POP => {
                    last = self.pop();
                }
                OpCode::POPN(n) => self.popn(*n), //TODO here's that 255 local limit again
                OpCode::GOTO_IF_FALSE(offset) => {
                    let value = self.peek();
                    // println!("GOTO_IF_FALSE: {}", value);
                    if !Self::is_truthy(value) {
                        frame.forward(*offset);
                    }
                }
                OpCode::GOTO_IF_TRUE(offset) => {
                    let value = self.peek();
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
                OpCode::CALL(param_count) => {
                    let value = self.peekn(*param_count);
                    devout!("CALL: {}", value);
                    if let Value::Function(f) = value {
                        // let vv = f.as_ref();
                        // let new_frame =
                        // drop(frame);
                        // drop(self.ip);

                        // frame = &mut dummy_frame;
                        let frame_top = unsafe { self.stack_top.sub((*param_count as usize) + 1) };
                        let new_frame = CallFrame::new(
                            f.clone(),
                            self.stack_count - (*param_count as usize) - 1,
                        );
                        devout!("current stack count {}", frame.stack_snapshot);
                        frames.push(new_frame);
                        frame_count += 1;
                        frame = frames.last_mut().unwrap();

                        frame.local_stack = frame_top;
                        devout!("top of frame stack {}", unsafe { &*frame.local_stack });
                        // frame.ip = f.chunk.code.as_ptr();
                        // // frame.stack.resize(256, Value::Nil); // TODO
                        // self.push(Value::Function(f.clone())); // TODO this needs to store the function object itself somehow, RC?

                        // self.frames.push(frame);
                    } else {
                        return Err(SiltError::NotCallable(format!("Value: {}", value)));
                    }
                }
                OpCode::PRINT => {
                    println!("<<<<<< {} >>>>>>>", self.pop());
                }
                OpCode::META(_) => todo!(),
            }
            frame.iterate();
            //stack
            #[cfg(feature = "dev-out")]
            {
                self.print_stack(&frame);
                println!("---");
            }
        }
    }

    // TODO is having a default empty chunk cheaper?
    /** We're operating on the assumtpion a chunk is always present when using this */
    fn get_chunk(frame: &CallFrame) -> &Chunk {
        &frame.function.chunk
    }

    // pub fn reset_stack(&mut self) {
    //     // TODO we probably dont even need to clear the stack, just reset the stack_top
    //     // self.stack.clear();
    //     // set to 0 index of stack
    //     self.stack_top = unsafe { self.stack.as_mut_ptr() };
    // }

    fn is_truthy(v: &Value) -> bool {
        // println!("is_truthy: {}", v);
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
                Operator::Less,
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
                Operator::Greater,
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

    pub fn print_stack(&self, frame: &CallFrame) {
        println!("=== Stack ({}) ===", self.stack_count);
        // 0 to stack_top
        print!("[");
        let mut c = 0;
        for i in self.stack.iter() {
            c += 1;
            if c > self.stack_count {
                break;
            }
            let s = format!("{:?}", i);
            if s == "nil" {
                // break;
                print!("_");
            } else {
                print!("{} ", i);
            }
            // print!("{} ", i);
        }
        print!("]");
        // println!("");
        // println!("stack_top: {:?}", self.stack_top);
        println!("---");
    }
}
