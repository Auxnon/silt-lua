use crate::{environment::Environment, statement::Statement, value::Value};

trait Callable {
    fn call(&self, global: &mut Environment, args: Vec<Value>) -> Value;
}

pub struct Function {
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

impl Function {
    pub fn new(params: Vec<String>, body: Vec<Statement>) -> Self {
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
