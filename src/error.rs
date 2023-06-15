use crate::{
    token::{Operator, Token},
    value::Value,
};

pub enum SiltError {
    //parse errors
    InvalidNumber(String),
    NotANumber(String),
    UnexpectedCharacter(char),
    UnterminatedString,
    UnterminatedParenthesis(usize, usize),
    InvalidTokenPlacement(Token),
    InvalidColonPlacement, // more specific to types and calls
    ExpectedLocalIdentifier,
    InvalidAssignment(Token),
    UnterminatedBlock,
    ExpectedThen,
    ExpectedDo,
    ExpectedToken(Token),

    //expression errors
    ExpInvalidOperator(Operator),
    ExpInvalidBitwise(ErrorTypes),
    ExpOpValueWithValue(ErrorTypes, Operator, ErrorTypes),
    ExpInvalidNegation(ErrorTypes),
    EarlyEndOfFile,
    ExpInvalid,

    // statement errors

    //interpreted errors
    EvalNoInteger(ErrorTypes),
    NotCallable(String),
    Return(Value),
}
pub enum ErrorTypes {
    String,
    Number,
    Operator,
    Integer,
    Bool,
    Nil,
    Infinity,
    NativeFunction,
    Function,
}

pub type Location = (usize, usize);

impl std::fmt::Display for SiltError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Return(v) => write!(f, "~Return {}~", v),
            SiltError::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            SiltError::NotANumber(s) => write!(f, "Not a number: {}", s),
            SiltError::UnexpectedCharacter(c) => write!(f, "Unexpected character: {}", c),
            SiltError::UnterminatedString => write!(f, "Unterminated string"),
            SiltError::UnterminatedParenthesis(x, y) => {
                write!(
                    f,
                    "Expected closing paren due to open paren '(' here {}:{}",
                    x, y
                )
            }
            SiltError::ExpInvalidOperator(t) => write!(f, "Invalid expression token: {}", t),
            SiltError::EarlyEndOfFile => write!(f, "File ended early"),
            SiltError::ExpOpValueWithValue(v1, op, v2) => {
                write!(f, "Cannot {} '{}' and '{}'", op, v1, v2)
            }
            SiltError::ExpInvalidNegation(v) => write!(f, "Cannot negate '{}'", v),
            SiltError::InvalidTokenPlacement(t) => write!(f, "Invalid token placement: {}", t),
            SiltError::InvalidColonPlacement => {
                write!(f, "Colon must be followed by type and assigned or a call")
            }
            SiltError::ExpInvalidBitwise(v) => write!(f, "Cannot bitwise on '{}'", v),
            SiltError::EvalNoInteger(v) => {
                write!(f, "{} has no direct integer conversion for operation", v)
            }
            SiltError::ExpectedLocalIdentifier => {
                write!(f, "Expected identifier following local keyword")
            }
            SiltError::InvalidAssignment(t) => {
                write!(f, "Cannot use assignment operator '{}' on declaration", t)
            }
            SiltError::UnterminatedBlock => write!(f, "Unterminated block"),
            SiltError::ExpectedThen => write!(f, "Expected 'then' after if condition"),
            SiltError::ExpectedDo => write!(f, "Expected 'do' after while condition"),
            Self::ExpectedToken(t) => write!(f, "Expected token: {}", t),
            Self::NotCallable(s) => write!(f, "Value '{}' is not callable", s),
            Self::ExpInvalid => write!(f, "Invalid expression"),
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
            ErrorTypes::NativeFunction => write!(f, "native_function"),
            ErrorTypes::Function => write!(f, "function"),
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
