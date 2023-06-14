use std::f32::consts::E;

use crate::{
    error::Location,
    token::{Operator, Token},
    value::Value,
};

pub enum Expression {
    Binary {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
    },
    Unary {
        operator: Operator,
        right: Box<Expression>,
    },
    // Primary {
    //     literal: Token,
    // },
    Literal {
        value: Value,
    },
    GroupingExpression {
        expression: Box<Expression>,
    },
    Variable {
        ident: usize,
    },
    Assign {
        ident: usize,
        value: Box<Expression>,
    },
    Logical {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
    },

    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
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
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::Logical {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::Unary { operator, right } => write!(f, "({} {})", operator, right),
            Expression::Literal { value } => write!(f, " {} ", value),
            Expression::GroupingExpression { expression } => write!(f, "G({})", expression),
            Expression::Variable { ident } => write!(f, "{}", ident),
            Expression::Assign { ident, value } => write!(f, "({} := {})", ident, value),
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
