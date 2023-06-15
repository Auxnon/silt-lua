use std::{f32::consts::E, rc::Rc};

use crate::{
    error::Location,
    function::Function,
    statement::Statement,
    token::{Operator, Token},
    value::Value,
};

pub enum Expression {
    Binary {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },
    Unary {
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },
    // Primary {
    //     literal: Token,
    // },
    Literal {
        value: Value,
        location: Location,
    },
    GroupingExpression {
        expression: Box<Expression>,
        location: Location,
    },
    Variable {
        ident: usize,
        location: Location,
    },
    Assign {
        ident: usize,
        value: Box<Expression>,
        location: Location,
    },
    Logical {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },

    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
        location: Location,
    },
    Function {
        value: Rc<Function>,
        location: Location,
    },
    // GetExpression {
    //     object: Box<Expression>,
    //     name: Token,
    // },
    // SetExpression {
    //     object: Box<Expression>,
    //     name: Token,
    //     value: Box<Expression>,
    // },
    // ThisExpression {
    //     keyword: Token,
    // },
    // SuperExpression {
    //     keyword: Token,
    //     method: Token,
    // },
    InvalidExpression,
}
impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::Binary {
                left,
                operator,
                right,
                location,
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::Logical {
                left,
                operator,
                right,
                location,
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::Unary {
                operator,
                right,
                location,
            } => write!(f, "({} {})", operator, right),
            Expression::Literal { value, location } => write!(f, " {} ", value),
            Expression::GroupingExpression {
                expression,
                location,
            } => write!(f, "G({})", expression),
            Expression::Variable { ident, location } => write!(f, "{}", ident),
            Expression::Assign {
                ident,
                value,
                location,
            } => write!(f, "({} := {})", ident, value),
            Expression::Call {
                callee,
                args: arguments,
                ..
            } => {
                let mut s = format!("({}(", callee);
                for arg in arguments {
                    s.push_str(&format!("{},", arg));
                }
                s.push_str("))");
                write!(f, "{}", s)
            }
            Expression::Function { value, .. } => write!(f, "function"),

            // Expression::Function(_, _) => write!(f, "function"),

            // Expression::Function { params, block } => {
            //     let mut s = format!("(fn(");
            //     for param in params {
            //         s.push_str(&format!("{},", param));
            //     }
            //     s.push_str(") {");
            //     for statement in block {
            //         s.push_str(&format!("\n||{}", statement));
            //     }
            //     s.push_str("})");
            //     write!(f, "{}", s)
            // }
            // Expression::AssignmentExpression { name, value } => {
            //     write!(f, "({} := {})", name, value)
            // }
            // Expression::LogicalExpression {
            //     left,
            //     operator,
            //     right,
            // } => write!(f, "({} {} {})", operator, left, right),
            // Expression::CallExpression { callee, arguments } => {
            //     let mut s = format!("({}(", callee);
            //     for arg in arguments {
            //         s.push_str(&format!("{},", arg));
            //     }
            //     s.push_str("))");
            //     write!(f, "{}", s)
            // }
            // Expression::GetExpression { object, name } => write!(f, "({}.{})", object, name),
            // Expression::SetExpression {
            //     object,
            //     name,
            //     value,
            // } => write!(f, "({}.{}={})", object, name, value),
            // Expression::ThisExpression { keyword } => write!(f, "{}", keyword),
            // Expression::SuperExpression { keyword, method } => write!(f, "{}.{}", keyword, method),
            // Expression::EndOfFile => write!(f, "EOF"),
            Expression::InvalidExpression => write!(f, "!Invalid_Expression!"),
        }
    }
}
