use crate::{token::Operator, value::Value};

pub enum SiltError {
    InvalidNumber(String),
    NotANumber(String),
    UnexpectedCharacter(char),
    UnterminatedString,

    UnterminatedParenthesis,
    InvalidExpressionOperator(Operator),
    ExpOpValueWithValue(ErrorTypes, Operator, ErrorTypes),
    ExpInvalidNegation(Value),
    EarlyEndOfFile,
}
pub enum ErrorTypes {
    String,
    Number,
    Operator,
    Integer,
    Bool,
    Nil,
    Infinity,
}

pub type Location = (usize, usize);

impl std::fmt::Display for SiltError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SiltError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            SiltError::NotANumber(s) => write!(f, "Not a number: {}", s),
            SiltError::UnexpectedCharacter(c) => write!(f, "Unexpected character: {}", c),
            SiltError::UnterminatedString => write!(f, "Unterminated string"),
            SiltError::UnterminatedParenthesis => {
                write!(f, "Expected closing paren ')' after expression")
            }
            SiltError::InvalidExpressionOperator(t) => write!(f, "Invalid expression token: {}", t),
            SiltError::EarlyEndOfFile => write!(f, "File ended early"),
            SiltError::ExpOpValueWithValue(v1, op, v2) => {
                write!(f, "Cannot {} '{}' and '{}'", op, v1, v2)
            }
            SiltError::ExpInvalidNegation(v) => write!(f, "Cannot negate '{}'", v),
        }
    }
}
impl std::fmt::Display for ErrorTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorTypes::String => write!(f, "string"),
            ErrorTypes::Number => write!(f, "number"),
            ErrorTypes::Operator => write!(f, "operator"),
            ErrorTypes::Integer => write!(f, "integer"),
            ErrorTypes::Bool => write!(f, "bool"),
            ErrorTypes::Nil => write!(f, "nil"),
            ErrorTypes::Infinity => write!(f, "infinity"),
        }
    }
}

pub struct ErrorTuple {
    pub code: SiltError,
    pub location: Location,
}

impl std::fmt::Display for ErrorTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.code, self.location.0, self.location.1)
    }
}
