use std::rc::Rc;

use crate::{error::Location, function::Function, token::Operator, value::Value};

/** first usize is mapped to environment hash for name, can be reversed for debug. Second usize is scope depth */
pub type Ident = (usize, usize);

#[derive(Clone)]
pub enum Expression {
    /** expression + expression */
    Binary {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },
    /** +expression | !expression */
    Unary {
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },
    /** true | 1 | nil | etc*/
    Literal { value: Value, location: Location },
    /** ( expression ) */
    GroupingExpression {
        expression: Box<Expression>,
        location: Location,
    },
    /** get <- ident */
    Variable { ident: Ident, location: Location },
    /** put expression -> ident */
    Assign {
        ident: Ident,
        value: Box<Expression>,
        location: Location,
    },
    /** expression OR expression AND expression */
    Logical {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
        location: Location,
    },
    /** ident() */
    Call {
        callee: Box<Expression>,
        args: Vec<Expression>,
        location: Location,
    },
    /** <- fn(){} */
    Function {
        value: Rc<Function>,
        location: Location,
    },
    /** shrug  */
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
            Expression::Variable { ident, location } => write!(f, "{}:{}", ident.0, ident.1),
            Expression::Assign {
                ident,
                value,
                location,
            } => write!(f, "({}:{} := {})", ident.0, ident.1, value),
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

            Expression::InvalidExpression => write!(f, "!Invalid_Expression!"),
        }
    }
}
