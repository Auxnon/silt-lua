use std::{borrow::Borrow, rc::Rc};

use hashbrown::HashMap;

use crate::{
    error::{ErrorTuple, Location, SiltError},
    expression::{self, Expression, Ident},
    function::{self, Function},
    statement::Statement,
};

type ResolverScope = HashMap<usize, bool>;
pub struct Resolver {
    scopes: Vec<ResolverScope>,
    errors: Vec<ErrorTuple>,
}
impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }
    pub fn process(&mut self, statements: &mut Vec<Statement>) {
        for statement in statements {
            self.resolve_statement(statement);
        }
    }
    pub fn resolve_statement(&mut self, statement: &mut Statement) {
        match statement {
            Statement::Expression(exp) => self.resolve(exp.as_mut()),
            Statement::Print(exp) => self.resolve(exp.as_mut()),
            Statement::Declare { ident, local, expr } => {
                // TODO seperating declare and define may only be necessary to chase down self assignment variables like local a=a
                // But this is probably not necessary in lua  or luau strict mode as functions need to self reference
                // and we have no simple way to distinguish between a function and a variable declaration here
                self.declare(ident.0);
                self.define(ident.0);
                self.resolve(expr.as_mut());
            }
            _=>{}
            // Statement::Block(s) => {
            //     self.start_scope();
            //     self.process(s);
            //     self.end_scope();
            // }
            // Statement::If {
            //     cond,
            //     then,
            //     else_cond,
            // } => {
            //     // TODO should be scoped
            //     self.resolve(*cond);
            //     self.process(then);
            //     if let Some(else_cond) = else_cond {
            //         self.process(else_cond);
            //     }
            // }
            // Statement::While { cond, block } => {
            //     // TODO should be scoped
            //     self.resolve(*cond);
            //     self.process(block);
            // }
            // Statement::NumericFor {
            //     ident,
            //     start,
            //     end,
            //     step,
            //     block,
            // } => todo!(),
            // Statement::Return(exp) => self.resolve(*exp),
            // Statement::Skip => todo!(),
            // Statement::InvalidStatement => todo!(),
        }
    }

    fn resolve(&mut self, expression: &mut Expression) {
        if self.scopes.is_empty() {
            return;
        }
        match expression {
            Expression::Binary {
                left,
                operator,
                right,
                location,
            } => {
                self.resolve(&mut *left);
                self.resolve(&mut *right);
            }
            Expression::Logical {
                left,
                operator,
                right,
                location,
            } => {
                self.resolve(&mut *left);
                self.resolve(&mut *right);
            }
            Expression::Unary {
                operator,
                right,
                location,
            } => {
                self.resolve(&mut *right);
            }
            Expression::Literal { value, location } => {}
            Expression::GroupingExpression {
                expression,
                location,
            } => {
                self.resolve(&mut *expression);
            }
            Expression::Variable {
                mut ident,
                location,
            } => {
                // if let Some(scope) = self.scopes.last() {
                //     if scope.get(&ident) == Some(&false) {
                //         self.error(SiltError::ResReadInOwnInit, location);
                //     }
                // }
                self.resolve_local(&mut ident);
                // let mut found = false;
                // for scope in self.scopes.iter().rev() {
                //     if scope.contains_key(&ident) {
                //         found = true;
                //         break;
                //     }
                // }
                // if !found {
                //     self.env.declare_global(self.env.to_register(&ident), expression::NIL);
                // }
            }
            Expression::Assign {
                mut ident,
                value,
                location,
            } => {
                self.resolve(&mut *value);
                self.resolve_local(&mut ident)
            }
            Expression::Call {
                callee,
                args: arguments,
                ..
            } => {
                self.resolve(&mut *callee);
                for arg in arguments {
                    self.resolve(arg);
                }
            }
            Expression::Function { value, .. } => {
                // self.declare(value.ident);
                // self.resolve_function(&mut value);

                self.resolve_function(value);
            }
            Expression::InvalidExpression => {}
            _ => {}
        }
    }
    fn error(&mut self, code: SiltError, location: Location) {
        self.errors.push(ErrorTuple { code, location });
    }
    fn start_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn end_scope(&mut self) {
        self.scopes.pop();
    }
    fn declare(&mut self, ident: usize) {
        if self.scopes.is_empty() {
            return;
        }

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(ident, false);
        }
    }
    fn define(&mut self, ident: usize) {
        if self.scopes.is_empty() {
            return;
        }

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(ident, true);
        }
    }

    fn resolve_local(&mut self, ident: &mut Ident) {
        for (i, scope) in self.scopes.iter().enumerate().rev() {
            if scope.contains_key(&ident.0) {
                *ident = (ident.0, i);
                // self.env.resolve_local(ident, self.scopes.len() - 1 - i);
                // self.env.scope.set(*ident, v, true, *local);
                // Interpreter::resovle(self.env, ident, i);
                // self.env.resolve_local(ident, i);
                return;
            }
        }
    }
    fn resolve_function(&mut self, function: &mut Rc<Function>) {
        self.start_scope();
        for param in function.params.iter() {
            self.declare(*param);
            self.define(*param);
        }

        // make a new function with the processed statements

        // let processed: Vec<Statement> = function
        //     .body
        //     .iter()
        //     .map(|s| {
        //         self.resolve_statement(s);
        //         s
        //     })
        //     .collect();

        // let new = Rc::new(function::Function::new(function.params.clone(), processed));

        let body = function.body.clone();

        body.iter()
            .for_each(|s| self.resolve_statement(&mut s.clone()));

        *function = Rc::new(function::Function::new(function.params.clone(), body));
        // let mut new: Function = (**function)
        //     .body
        //     .iter()
        //     .map(|s| {
        //         let s2 = s.clone();

        //         self.resolve_statement(s);
        //         s
        //     })
        //     .collect();
        // new.body = processed;
        // let new = Rc::new(new);

        // self.process(function.body);
        self.end_scope();
    }
}
