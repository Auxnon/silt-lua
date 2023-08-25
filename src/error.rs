use crate::{
    token::{Operator, Token},
    value::{self, Value},
};

#[derive(Clone)]
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
    ExpectedLabelIdentifier,
    ExpectedGotoIdentifier,
    UndefinedLabel(String),
    InvalidAssignment(Token),
    UnterminatedBlock,
    ExpectedThen,
    ExpectedDo,
    ExpectedToken(Token),
    TooManyLocals,
    TooManyOperations,
    TooManyParameters,
    ChunkCorrupt,

    //expression errors
    ExpInvalidOperator(Operator),
    ExpInvalidBitwise(ErrorTypes),
    ExpOpValueWithValue(ErrorTypes, Operator, ErrorTypes),
    ExpInvalidNegation(ErrorTypes),
    EarlyEndOfFile,
    ExpInvalid,

    // resolver errors
    // ResReadInOwnInit,

    // statement errors

    //interpreted errors
    EvalNoInteger(ErrorTypes),
    NotCallable(String),
    Return(Value),

    //vm
    VmCompileError,
    VmRuntimeError,
    VmCorruptConstant,
    VmUpvalueResolveError,

    Unknown,
}

#[derive(Debug, Clone)]
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
    Closure,
    Table,
}

pub type Location = (usize, usize);

impl std::fmt::Display for SiltError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VmUpvalueResolveError => {
                write!(f, "Unexpected issue resolving upvalue for closure")
            }
            Self::UndefinedLabel(s) => write!(f, "No matching goto label for '{}'", s),
            Self::ExpectedGotoIdentifier => write!(f, "Expected identifier following goto keyword"),
            Self::ChunkCorrupt => write!(f, "Invalid chunk due compilation corruption"),
            Self::TooManyOperations => write!(
                f,
                "Too many operations within this condition, limited to 65535"
            ),
            Self::TooManyLocals => write!(f, "Too many local variables, limited to 255"),
            Self::TooManyParameters => write!(f, "Too many parameters, limited to 255"),
            Self::Return(v) => write!(f, "~Return {}~", v),
            Self::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            Self::NotANumber(s) => write!(f, "Not a number: {}", s),
            Self::UnexpectedCharacter(c) => write!(f, "Unexpected character: {}", c),
            Self::UnterminatedString => write!(f, "Unterminated string"),
            Self::UnterminatedParenthesis(x, y) => {
                write!(
                    f,
                    "Expected closing paren due to open paren '(' here {}:{}",
                    x, y
                )
            }
            Self::ExpInvalidOperator(t) => write!(f, "Invalid expression token: {}", t),
            Self::EarlyEndOfFile => write!(f, "File ended early"),
            Self::ExpOpValueWithValue(v1, op, v2) => {
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
            Self::ExpectedLabelIdentifier => {
                write!(f, "Expected identifier only inbetween label tokens `::`")
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
            Self::VmCompileError => write!(f, "Error compiling chunk"),
            Self::VmRuntimeError => write!(f, "Runtime error for chunk"),
            Self::VmCorruptConstant => write!(f, "Constant store corrupted"),
            Self::Unknown => write!(f, "Unknown error"),
            // Self::ResReadInOwnInit => write!(f, "Cannot read variable in its own initializer"),
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
            ErrorTypes::Closure => write!(f, "(function)"),
            ErrorTypes::Table => write!(f, "table"),
        }
    }
}

#[derive(Clone)]
pub struct ErrorTuple {
    pub code: SiltError,
    pub location: Location,
}

impl std::fmt::Display for ErrorTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.code, self.location.0, self.location.1)
    }
}
