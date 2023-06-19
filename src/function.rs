use std::rc::Rc;

use crate::{
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
