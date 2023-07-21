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
                                      // TODO slots: Vec<Value>, should point into VM values stack somehoew
}
impl CallFrame<'_> {
    pub fn get_instruction(&self) -> &crate::code::OpCode {
        &self.function.chunk.code[self.ip]
    }
}
#[derive(Default)]
pub struct FunctionObject {
    pub is_script: bool,
    pub name: Option<String>,
    pub chunk: Chunk,
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
