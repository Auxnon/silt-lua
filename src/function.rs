use std::rc::Rc;

use crate::{
    chunk::Chunk,
    environment::{Environment, Scope},
    statement::Statement,
    value::Value,
};

trait Callable {
    fn call(&self, global: &mut Environment, args: Vec<Value>) -> Value;
}

pub struct Function {
    pub params: Vec<usize>,
    pub body: Vec<Statement>,
}

impl Function {
    pub fn new(params: Vec<usize>, body: Vec<Statement>) -> Self {
        Self { params, body }
    }
}

pub struct ScopedFunction {
    pub func: Rc<Function>,
    pub scope: Vec<Scope>,
}

/////////////
///
pub struct CallFrame<'a> {
    pub function: &'a FunctionObject, // pointer
    pub ip: usize,                    // TODO pointer to caller's ip
    // ip: *const OpCode
    // pub base: usize,
    pub stack: Vec<Value>, // should point into VM values stack somehoew
}

impl<'a> CallFrame<'a> {
    pub fn new(function: &'a FunctionObject) -> Self {
        Self {
            function,
            ip: 0,
            stack: Vec::new(),
        }
    }

    pub fn current_instruction(&self) -> &crate::code::OpCode {
        &self.function.chunk.code[self.ip]
    }
    pub fn iterate(&mut self) {
        self.ip += 1;
    }
    pub fn set_val(&mut self, index: u8, value: Value) {
        self.stack[index as usize] = value;
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

    pub fn push(&mut self, value: Value) {
        self.stack.push(value);
    }

    /** pop and return top of stack */
    pub fn pop(&mut self) -> Value {
        self.stack.pop().unwrap()
    }

    /** pop N number of values from stack */
    pub fn popn(&mut self, n: u8) {
        self.stack.truncate(self.stack.len() - n as usize);
    }

    /** take and replace with a Nil */
    pub fn take(&mut self) -> Value {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        let v = self.pop();
        self.push(Value::Nil);
        v
    }
    pub fn duplicate(&self) -> Value {
        match self.peek() {
            Some(v) => v.clone(),
            None => Value::Nil,
        }
    }
    // TODO too safe, see below
    pub fn peek(&self) -> Option<&Value> {
        self.stack.last()
    }

    // TODO may as well make it unsafe, our compiler should take the burden of correctness
    pub fn peek0(&self) -> &Value {
        // unsafe { *self.stack_top.sub(1) }
        self.stack.last().unwrap()
    }

    // TODO validate safety of this, compiler has to be solid af!
    pub fn forward(&mut self, offset: u16) {
        self.ip += offset as usize;
        // self.ip = unsafe { self.ip.add(offset as usize) };
    }

    pub fn rewind(&mut self, offset: u16) {
        self.ip -= offset as usize;
        // self.ip = unsafe { self.ip.sub(offset as usize) };
        // println!("rewind: {}", unsafe { &*self.ip });
    }
}
#[derive(Default)]
pub struct FunctionObject {
    pub is_script: bool,
    pub name: Option<String>,
    pub chunk: Chunk,
    // pub upvalues: Vec<usize>,
    // pub arity: usize,
}

impl FunctionObject {
    pub fn new(name: Option<String>, is_script: bool) -> Self {
        Self {
            name,
            is_script,
            chunk: Chunk::new(),
        }
    }
    pub fn set_chunk(&mut self, chunk: Chunk) {
        self.chunk = chunk;
    }
}

impl ScopedFunction {
    pub fn new(scope: Vec<Scope>, func: Rc<Function>) -> Self {
        Self { func, scope }
    }
}

impl Callable for Function {
    fn call(&self, global: &mut Environment, args: Vec<Value>) -> Value {
        // let mut env = Environment::new();

        // for (i, arg) in self.params.iter().enumerate() {
        //     env.define(arg, args[i].clone());
        // }
        // match self.body.evaluate(&mut env) {
        //     Ok(v) => v,
        //     Err(e) => {
        //         eprintln!("{}", e);
        //         Value::Nil
        //     }
        // }
        Value::Nil
    }
}
