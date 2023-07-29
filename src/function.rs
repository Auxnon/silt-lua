use std::rc::Rc;

use crate::{
    chunk::Chunk,
    code::OpCode,
    environment::{Environment, Scope},
    silt::SiltLua,
    statement::Statement,
    value::Value,
};

/////////////
///
pub struct CallFrame {
    pub function: Rc<FunctionObject>, // pointer
    // ip: *const OpCode
    // pub base: usize,
    // pointer point sinto VM values stack
    pub stack_snapshot: usize,
    pub local_stack: *mut Value,
    pub ip: *const OpCode,
}

impl<'frame> CallFrame {
    pub fn new(function: Rc<FunctionObject>, stack_snapshot: usize) -> Self {
        let ip = function.chunk.code.as_ptr();
        Self {
            function,
            ip,
            local_stack: std::ptr::null_mut(),
            stack_snapshot,
        }
    }

    pub fn current_instruction(&self) -> &crate::code::OpCode {
        // &self.function.chunk.code[self.ip]
        unsafe { &*self.ip }
    }
    pub fn iterate(&mut self) {
        // self.ip += 1;
        self.ip = unsafe { self.ip.add(1) };
    }
    pub fn set_val(&mut self, index: u8, value: Value) {
        // self.stack[index as usize] = value;
        unsafe { *self.local_stack.add(index as usize) = value };
    }

    pub fn get_val(&self, index: u8) -> &Value {
        // &self.stack[index as usize]
        // println!("get_val: {}", index);
        // println!("top: {}", unsafe { &*self.local_stack });
        unsafe { &*self.local_stack.add(index as usize) }
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
    pub fn take(&mut self) -> &Value {
        // self.stack_top = unsafe { self.stack_top.sub(1) };
        // unsafe { *self.stack_top }
        let v = unsafe { &*self.local_stack };
        unsafe { *self.local_stack = Value::Nil };
        v
    }

    // TODO validate safety of this, compiler has to be solid af!
    pub fn forward(&mut self, offset: u16) {
        // self.ip += offset as usize;
        self.ip = unsafe { self.ip.add(offset as usize) };
    }

    pub fn rewind(&mut self, offset: u16) {
        // self.ip -= offset as usize;
        self.ip = unsafe { self.ip.sub(offset as usize) };
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

// NATIVE
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

pub struct NativeObject {
    name: String,
    pub function: fn(&mut SiltLua, Vec<Value>) -> Value,
}

impl NativeObject {
    pub fn new(name: String, function: fn(&mut SiltLua, Vec<Value>) -> Value) -> Self {
        Self { name, function }
    }
}
