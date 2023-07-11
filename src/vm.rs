use crate::{
    chunk::Chunk,
    code::OpCode,
    error::{ErrorTypes, SiltError},
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
    ($vm:ident, $op:tt, $opp:tt) => {
        {

            // TODO test speed of this vs 1 pop and a mutate
            let r = $vm.pop();
            let l = $vm.pop();

            $vm.push(match (l, r) {
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
    chunk: Option<&'a Chunk>,
    /** Instruction to be run at start of loop  */
    ip: *const OpCode, // TODO usize vs *const OpCode, will rust optimize the same?
    stack: Vec<Value>, // TODO fixed size array vs Vec
    /** Next empty location */
    stack_top: *mut Value,
    globals: hashbrown::HashMap<String, Value>, // TODO store strings as identifer usize and use that as key
                                                // references: Vec<Reference>,
                                                // obj
                                                // TODO should we store all strings in their own table/array for better equality checks? is this cheaper?
}

impl<'a> VM<'a> {
    pub fn new() -> Self {
        // TODO try the hard way
        // force array to be 256 Values
        // let stack = unsafe {
        //     std::alloc::alloc(std::alloc::Layout::array::<Value>(256).unwrap()) as *mut [Value; 256]
        // };
        // let stack_top = stack as *mut Value;
        let stack = vec![];
        let stack_top = stack.as_ptr() as *mut Value;
        Self {
            chunk: None,
            ip: 0 as *const OpCode,
            stack, //: unsafe { *stack },
            stack_top,
            globals: hashbrown::HashMap::new(),
        }
    }

    pub fn interpret(&mut self, chunk: &'a Chunk) -> Result<Value, SiltError> {
        self.ip = chunk.code.as_ptr();
        self.chunk = Some(chunk);
        self.run()
    }

    fn run(&mut self) -> Result<Value, SiltError> {
        let mut last = Value::Nil; // TODO temporary for testing
        loop {
            let instruction = unsafe { &*self.ip };

            devout!("ip: {:p} | {}", self.ip, instruction);
            self.ip = unsafe { self.ip.add(1) };
            match instruction {
                OpCode::RETURN => {
                    // println!("<< {}", self.pop());
                    match self.stack.pop() {
                        Some(v) => return Ok(v),
                        None => return Ok(last),
                    }
                }
                OpCode::CONSTANT { constant } => {
                    let value = self.chunk.unwrap().get_constant(*constant);
                    devout!("CONSTANT value {}", value);
                    self.push(value.clone());
                    // match value {
                    //     Value::Number(f) => self.push(*f),
                    //     Value::Integer(i) => self.push(*i as f64),
                    //     _ => {}
                    // }
                }
                OpCode::DEFINE_GLOBAL { constant } => {
                    let value = self.chunk.unwrap().get_constant(*constant);
                    if let Value::String(s) = value {
                        let v = self.pop();
                        self.globals.insert(s.to_string(), v);
                    } else {
                        return Err(SiltError::VmRuntimeError);
                    }
                }
                // TODO does this need to exist?
                OpCode::SET_GLOBAL { constant } => {
                    let value = self.chunk.unwrap().get_constant(*constant);
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
                    let value = self.get_chunk().get_constant(*constant);
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
                    self.stack[*index as usize] = value;
                }
                OpCode::GET_LOCAL { index } => {
                    self.push(self.stack[*index as usize].clone());
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
                        Some(Value::Number(n)) => {
                            let f = -n;
                            self.pop();
                            self.push(Value::Number(f))
                        }
                        Some(Value::Integer(i)) => {
                            let f = -i;
                            self.pop();
                            self.push(Value::Integer(f))
                        }
                        None => Err(SiltError::EarlyEndOfFile)?,
                        Some(c) => Err(SiltError::ExpInvalidNegation(c.to_error()))?,
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
                    let value = self.peek0();
                    if !Self::is_truthy(value) {
                        self.forward(*offset);
                    }
                }
                OpCode::GOTO_IF_TRUE(offset) => {
                    let value = self.peek0();
                    if Self::is_truthy(value) {
                        self.forward(*offset);
                    }
                }
                OpCode::FORWARD(offset) => {
                    self.forward(*offset);
                }
                OpCode::REWIND(offset) => {
                    self.rewind(*offset);
                }
                OpCode::PRINT => {
                    println!("<<<<<< {} >>>>>>>", self.pop());
                }
                OpCode::META(_) => todo!(),
            }
            //stack
            #[cfg(feature = "dev-out")]
            {
                self.print_stack();
                println!("---");
            }
        }
    }

    // TODO is having a default empty chunk cheaper?
    /** We're operating on the assumtpion a chunk is always present when using this */
    fn get_chunk(&self) -> &Chunk {
        self.chunk.unwrap()
    }

    pub fn reset_stack(&mut self) {
        // TODO we probably dont even need to clear the stack, just reset the stack_top
        // self.stack.clear();
        // set to 0 index of stack
        self.stack_top = unsafe { self.stack.as_mut_ptr() };
    }

    pub fn push(&mut self, value: Value) {
        // TODO can we push to the stack by pointer? Or should we just push on a Vec?
        // *self.stack_top= value;

        // unsafe { *self.stack_top = value };
        // self.stack.push(value);
        // self.stack_top = self.stack.as_ptr().add(self.stack.len());

        // unsafe { *self.stack_top = value };
        // self.stack_top = unsafe { self.stack_top.add(1) };
        self.stack.push(value);
    }

    fn pop(&mut self) -> Value {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        self.stack.pop().unwrap()
    }
    fn popn(&mut self, n: u8) {
        self.stack.truncate(self.stack.len() - n as usize);
    }

    /** take and replace with a Nil */
    fn take(&mut self) -> Value {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        let v = self.pop();
        self.push(Value::Nil);
        v
    }

    fn duplicate(&mut self) -> Value {
        match self.peek() {
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }

    // TODO too safe, see below
    fn peek(&mut self) -> Option<&Value> {
        self.stack.last()
    }

    // TODO may as well make it unsafe, our compiler should take the burden of correctness
    fn peek0(&mut self) -> &Value {
        // unsafe { *self.stack_top.sub(1) }
        self.stack.last().unwrap()
    }

    fn peek1(&mut self) -> &Value {
        // unsafe { *self.stack_top.sub(2) }
        &self.stack[self.stack.len() - 2]
    }

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

    /// TODO validate safety of this, compiler has to be solid af!
    fn forward(&mut self, offset: u16) {
        self.ip = unsafe { self.ip.add(offset as usize) };
    }
    fn rewind(&mut self, offset: u16) {
        self.ip = unsafe { self.ip.sub(offset as usize) };
        // println!("rewind: {}", unsafe { &*self.ip });
    }

    /** unsafe as hell, we're relying on compiler*/
    fn read_string(&mut self, constant: u8) -> String {
        let value = self.chunk.unwrap().get_constant(constant);
        if let Value::String(s) = value {
            return s.to_string();
        } else {
            unreachable!("Only strings can be identifiers")
        }
    }

    pub fn print_stack(&self) {
        println!("=== Stack ===");
        // 0 to stack_top
        print!("[");
        for i in self.stack.iter() {
            print!("{} ", i);
        }
        print!("]");
        println!("");
        println!("stack_top: {:?}", self.stack_top);
        println!("---");
    }
}
