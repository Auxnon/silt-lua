use crate::{chunk::Chunk, code::OpCode, error::SiltError, token::Operator, value::Value};

pub struct VM<'a> {
    chunk: Option<&'a Chunk>,
    /** Instruction to be run at start of loop  */
    ip: *const OpCode, // TODO usize vs *const OpCode, will rust optimize the same?
    stack: Vec<Value>, // TODO fixed size array vs Vec
    /** Next empty location */
    stack_top: *mut Value,
}
macro_rules! binary_op {
    ($vm:ident, $op:tt, $opp:tt) => {
        {
        let r = $vm.pop();
        let l = $vm.pop();

        match (l, r) {
            (Value::Number(left), Value::Number(right)) => $vm.push(Value::Number(left $op right)),
            (Value::Integer(left), Value::Integer(right)) => $vm.push(Value::Integer(left $op right)),
            (Value::Number(left), Value::Integer(right)) => $vm.push(Value::Number(left $op right as f64)),
            (Value::Integer(left), Value::Number(right)) => $vm.push(Value::Number(left as f64 $op right)),
            (ll,rr) => return Err(SiltError::ExpOpValueWithValue(ll.to_error(), Operator::$opp, rr.to_error()))
        }

        }
    };
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
        }
    }

    pub fn interpret(&mut self, chunk: &'a Chunk) -> Result<(), SiltError> {
        self.ip = chunk.code.as_ptr();
        self.chunk = Some(chunk);
        self.run()
    }
    fn run(&mut self) -> Result<(), SiltError> {
        loop {
            let instruction = unsafe { &*self.ip };
            // TODO devout
            println!("ip: {:p} | {}", self.ip, instruction);
            self.ip = unsafe { self.ip.add(1) };
            match instruction {
                OpCode::RETURN => {
                    println!("<< {}", self.pop());
                    return Ok(());
                }
                OpCode::CONSTANT { constant } => {
                    let value = self.chunk.unwrap().get_constant(*constant);
                    // TODO devout
                    println!("CONSTANT value {}", value);
                    self.push(value.clone());
                    // match value {
                    //     Value::Number(f) => self.push(*f),
                    //     Value::Integer(i) => self.push(*i as f64),
                    //     _ => {}
                    // }
                }
                OpCode::ADD => binary_op!(self, +, Add),
                OpCode::SUB => binary_op!(self, -, Sub),
                OpCode::MULTIPLY => binary_op!(self, *, Multiply),
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
                OpCode::LITERAL { dest, literal } => {}
            }
            //stack
            self.print_stack();
            println!("---");
        }
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
    fn peek(&mut self) -> Option<&Value> {
        self.stack.last()
    }
    fn peek0(&mut self) -> &Value {
        // unsafe { *self.stack_top.sub(1) }
        self.stack.last().unwrap()
    }
    fn peek1(&mut self) -> &Value {
        // unsafe { *self.stack_top.sub(2) }
        &self.stack[self.stack.len() - 2]
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
