use std::rc::Rc;

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
pub struct VM<'a> {
    body: &'a FunctionObject,
    frames: Vec<CallFrame<'a>>,
    /** Instruction to be run at start of loop  */
    // ip: *const OpCode, // TODO usize vs *const OpCode, will rust optimize the same?
    // stack: Vec<Value>, // TODO fixed size array vs Vec
    /** Next empty location */
    // stack_top: *mut Value,
    globals: hashbrown::HashMap<String, Value>, // TODO store strings as identifer usize and use that as key
                                                // references: Vec<Reference>,
                                                // obj
                                                // TODO should we store all strings in their own table/array for better equality checks? is this cheaper?
}

impl<'a> VM<'a> {
    pub fn new(body: &'a FunctionObject) -> Self {
        // TODO try the hard way
        // force array to be 256 Values
        // let stack = unsafe {
        //     std::alloc::alloc(std::alloc::Layout::array::<Value>(256).unwrap()) as *mut [Value; 256]
        // };
        // let stack_top = stack as *mut Value;
        // let stack = vec![];
        // let stack_top = stack.as_ptr() as *mut Value;
        Self {
            body,
            frames: vec![],
            // ip: 0 as *const OpCode,
            // stack, //: unsafe { *stack },
            // stack_top,
            globals: hashbrown::HashMap::new(),
        }
    }

    pub fn interpret(&mut self, object: Rc<FunctionObject>) -> Result<Value, SiltError> {
        // TODO param is a reference of &'a
        // self.ip = object.chunk.code.as_ptr();
        // frame.ip = object.chunk.code.as_ptr();
        // frame.slots = self.stack ???
        let mut frame = CallFrame::new(&object);
        // frame.stack.resize(256, Value::Nil); // TODO
        frame.stack.push(Value::Function(object)); // TODO this needs to store the function object itself somehow, RC?
        self.frames.push(frame);
        // self.body = object;
        self.run()
    }

    fn run(&mut self) -> Result<Value, SiltError> {
        let mut last = Value::Nil; // TODO temporary for testing

        let mut frame = self.frames.pop().unwrap();
        loop {
            // let instruction = unsafe { &*frame.ip };
            // self.ip = unsafe { self.ip.add(1) };
            let instruction = frame.current_instruction();

            // devout!("ip: {:p} | {}", self.ip, instruction);
            // let instruction = self.get_chunk().code.get()

            // TODO how much faster would it be to order these ops in order of usage, does match hash? probably.
            match instruction {
                OpCode::RETURN => {
                    // println!("<< {}", self.pop());
                    match frame.stack.pop() {
                        Some(v) => return Ok(v),
                        None => return Ok(last),
                    }
                }
                OpCode::CONSTANT { constant } => {
                    let value = self.get_chunk().get_constant(*constant);
                    devout!("CONSTANT value {}", value);
                    frame.push(value.clone());
                    // match value {
                    //     Value::Number(f) => self.push(*f),
                    //     Value::Integer(i) => self.push(*i as f64),
                    //     _ => {}
                    // }
                }
                OpCode::DEFINE_GLOBAL { constant } => {
                    let value = self.body.chunk.get_constant(*constant);
                    if let Value::String(s) = value {
                        let v = frame.pop();
                        self.globals.insert(s.to_string(), v);
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                // TODO does this need to exist?
                OpCode::SET_GLOBAL { constant } => {
                    let value = self.body.chunk.get_constant(*constant);
                    if let Value::String(s) = value {
                        let v = frame.duplicate();
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
                    let value = self.get_chunk().get_constant(*constant);
                    if let Value::String(s) = value {
                        if let Some(v) = self.globals.get(&**s) {
                            frame.push(v.clone());
                        } else {
                            frame.push(Value::Nil);
                        }
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                OpCode::SET_LOCAL { index } => {
                    let value = frame.duplicate();
                    // frame.stack[*index as usize] = value;
                    frame.set_val(*index, value)
                }
                OpCode::GET_LOCAL { index } => {
                    frame.push(frame.stack[*index as usize].clone());
                    // TODO ew cloning, is our cloning optimized yet?
                    // TODO also we should convert from stack to register based so we can use the index as a reference instead
                }
                OpCode::DEFINE_LOCAL { constant } => todo!(),
                OpCode::ADD => binary_op!(frame, +, Add),
                OpCode::SUB => binary_op!(frame,-, Sub),
                OpCode::MULTIPLY => binary_op!(frame,*, Multiply),
                OpCode::DIVIDE => {
                    let right = frame.pop();
                    let left = frame.pop();

                    match (left, right) {
                        (Value::Number(left), Value::Number(right)) => {
                            frame.push(Value::Number(left / right))
                        }
                        (Value::Integer(left), Value::Integer(right)) => {
                            frame.push(Value::Number(left as f64 / right as f64))
                        }
                        (Value::Number(left), Value::Integer(right)) => {
                            frame.push(Value::Number(left / right as f64))
                        }
                        (Value::Integer(left), Value::Number(right)) => {
                            frame.push(Value::Number(left as f64 / right))
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
                    match frame.peek() {
                        Some(Value::Number(n)) => {
                            let f = -n;
                            frame.pop();
                            frame.push(Value::Number(f))
                        }
                        Some(Value::Integer(i)) => {
                            let f = -i;
                            frame.pop();
                            frame.push(Value::Integer(f))
                        }
                        None => Err(SiltError::EarlyEndOfFile)?,
                        Some(c) => Err(SiltError::ExpInvalidNegation(c.to_error()))?,
                    }
                    // TODO  test this vs below: unsafe { *self.stack_top = -*self.stack_top };
                }
                OpCode::NIL => frame.push(Value::Nil),
                OpCode::TRUE => frame.push(Value::Bool(true)),
                OpCode::FALSE => frame.push(Value::Bool(false)),
                OpCode::NOT => {
                    let value = frame.pop();
                    frame.push(Value::Bool(!Self::is_truthy(&value)));
                }
                OpCode::EQUAL => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(Self::is_equal(&l, &r)));
                }
                OpCode::NOT_EQUAL => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(!Self::is_equal(&l, &r)));
                }
                OpCode::LESS => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(Self::is_less(&l, &r)?));
                }
                OpCode::LESS_EQUAL => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(!Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(Self::is_greater(&l, &r)?));
                }
                OpCode::GREATER_EQUAL => {
                    let r = frame.pop();
                    let l = frame.pop();
                    frame.push(Value::Bool(!Self::is_less(&l, &r)?));
                }
                OpCode::CONCAT => {
                    let r = frame.pop();
                    let l = frame.pop();
                    match (l, r) {
                        (Value::String(left), Value::String(right)) => {
                            frame.push(Value::String(Box::new(*left + &right)))
                        }
                        (Value::String(left), v2) => {
                            frame.push(Value::String(Box::new(*left + &v2.to_string())))
                        }
                        (v1, Value::String(right)) => {
                            frame.push(Value::String(Box::new(v1.to_string() + &right)))
                        }
                        (v1, v2) => {
                            frame.push(Value::String(Box::new(v1.to_string() + &v2.to_string())))
                        }
                    }
                }

                OpCode::LITERAL { dest, literal } => {}
                OpCode::POP => {
                    last = frame.pop();
                }
                OpCode::POPN(n) => frame.popn(*n), //TODO here's that 255 local limit again
                OpCode::GOTO_IF_FALSE(offset) => {
                    let value = frame.peek0();
                    if !Self::is_truthy(value) {
                        frame.forward(*offset);
                    }
                }
                OpCode::GOTO_IF_TRUE(offset) => {
                    let value = frame.peek0();
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
                OpCode::PRINT => {
                    println!("<<<<<< {} >>>>>>>", frame.pop());
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
    fn get_chunk(&self) -> &Chunk {
        &self.body.chunk
    }

    // pub fn reset_stack(&mut self) {
    //     // TODO we probably dont even need to clear the stack, just reset the stack_top
    //     // self.stack.clear();
    //     // set to 0 index of stack
    //     self.stack_top = unsafe { self.stack.as_mut_ptr() };
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
                Operator::Less,
                r.to_error(),
            ))?,
        })
    }

    fn is_greater(l: &Value, r: &Value) -> Result<bool, SiltError> {
        Ok(match (l, r) {
            (Value::Number(left), Value::Number(right)) => left > right,
            (Value::Integer(left), Value::Integer(right)) => left > right,
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

    /** unsafe as hell, we're relying on compiler*/
    fn read_string(&mut self, constant: u8) -> String {
        let value = self.get_chunk().get_constant(constant);
        if let Value::String(s) = value {
            return s.to_string();
        } else {
            unreachable!("Only strings can be identifiers")
        }
    }

    pub fn print_stack(&self, frame: &CallFrame<'a>) {
        println!("=== Stack ===");
        // 0 to stack_top
        print!("[");
        for i in frame.stack.iter() {
            print!("{} ", i);
        }
        print!("]");
        // println!("");
        // println!("stack_top: {:?}", self.stack_top);
        println!("---");
    }
}
