use std::fmt::write;

use crate::{
    token::{Operator, Token},
    userdata::MetaMethod,
    value::{self, Value},
};

#[derive(Clone, PartialEq, Debug)]
pub enum SiltError {
    //parse errors
    InvalidNumber(String),
    NotANumber(String),
    UnexpectedCharacter(char),
    UnterminatedString,
    UnterminatedParenthesis(usize, usize),
    UnterminatedBracket(usize, usize),
    InvalidTokenPlacement(Token),
    InvalidColonPlacement, // more specific to types and calls
    ExpectedLocalIdentifier,
    ExpectedLabelIdentifier,
    ExpectedGotoIdentifier,
    ExpectedFieldIdentifier,
    TableExpectedCommaOrCloseBrace,
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
    ExpInvalidOperator(MetaMethod),
    ExpInvalidBitwise(ValueTypes),
    ExpInvalidLength(ValueTypes),
    ExpOpValueWithValue(ValueTypes, MetaMethod, ValueTypes),
    ExpInvalidNegation(ValueTypes),
    EarlyEndOfFile,
    ExpInvalid,
    ExpectedAssign,

    // resolver errors
    // ResReadInOwnInit,

    // statement errors

    //interpreted errors
    EvalNoInteger(ValueTypes),
    NotCallable(String),
    // Return(Value),
    MetaMethodMissing(MetaMethod),
    MetaMethodNotCallable(MetaMethod),

    // Userdata errors
    UDNoInitField,
    UDNoInitMethod,
    UDNoFieldRef,
    UDNoMethodRef,
    UDTypeMismatch,

    //vm
    VmCompileError,
    VmRuntimeError,
    VmCorruptConstant,
    VmUpvalueResolveError,
    VmNonTableOperations(ValueTypes),

    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueTypes {
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
    UserData,
    #[cfg(feature = "vectors")]
    Vec3,
    #[cfg(feature = "vectors")]
    Vec2,
}

pub type Location = (usize, usize);

impl std::fmt::Display for SiltError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::VmUpvalueResolveError => {
                write!(f, "Unexpected issue resolving upvalue for closure")
            }
            Self::ExpectedAssign => write!(f, "Expected assignment operator '='"),
            Self::TableExpectedCommaOrCloseBrace => {
                write!(
                    f,
                    "Table expected a comma or termination by closing brace '}}'"
                )
            }
            Self::UndefinedLabel(s) => write!(f, "No matching goto label for '{}'", s),
            Self::ExpectedGotoIdentifier => write!(f, "Expected identifier following goto keyword"),
            Self::ExpectedFieldIdentifier => {
                write!(f, "Expected identifier following field accessor '.'")
            }
            Self::ChunkCorrupt => write!(f, "Invalid chunk due compilation corruption"),
            Self::TooManyOperations => write!(
                f,
                "Too many operations within this condition, limited to 65535"
            ),
            Self::TooManyLocals => write!(f, "Too many local variables, limited to 255"),
            Self::TooManyParameters => write!(f, "Too many parameters, limited to 255"),
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
            Self::UnterminatedBracket(x, y) => {
                write!(
                    f,
                    "Expected closing bracket due to open bracket '[' here {}:{}",
                    x, y
                )
            }
            Self::ExpInvalidOperator(t) => write!(f, "Invalid expression token: {}", t),
            Self::EarlyEndOfFile => write!(f, "File ended early"),
            Self::ExpOpValueWithValue(v1, op, v2) => {
                write!(f, "Cannot {} '{}' and '{}'", op, v1, v2)
            }
            Self::VmNonTableOperations(v) => {
                write!(
                    f,
                    "Cannot perform table operations on a non-table value: {}",
                    v
                )
            }
            SiltError::ExpInvalidNegation(v) => write!(f, "Cannot negate '{}'", v),
            SiltError::InvalidTokenPlacement(t) => write!(f, "Invalid token placement: {}", t),
            SiltError::InvalidColonPlacement => {
                write!(f, "Colon must be followed by type and assigned or a call")
            }
            SiltError::ExpInvalidBitwise(v) => write!(f, "Cannot bitwise on '{}'", v),
            Self::ExpInvalidLength(v) => write!(f, "Cannot get length of '{}'", v),
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
            SiltError::MetaMethodMissing(meta_method) => {
                write!(f, "Meta method missing for '{}'", meta_method)
            }
            SiltError::MetaMethodNotCallable(meta_method) => {
                write!(f, "Value for meta method '{}' is not callable", meta_method)
            }

            SiltError::UDNoInitField=> write!(f, "UserData field not setup"),
            SiltError::UDNoInitMethod=> write!(f, "UserData method not setup"),
            SiltError::UDNoFieldRef=> write!(f, "UserData field does not exist"),
            SiltError::UDNoMethodRef=> write!(f, "UserData method does not exist"),
            SiltError::UDTypeMismatch => {
                write!(f, "UserData type mismatch during method or field access")
            }
        }
    }
}
impl std::fmt::Display for ValueTypes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueTypes::String => write!(f, "string"),
            ValueTypes::Number => write!(f, "number"),
            ValueTypes::Operator => write!(f, "operator"),
            ValueTypes::Integer => write!(f, "integer"),
            ValueTypes::Bool => write!(f, "bool"),
            ValueTypes::Nil => write!(f, "nil"),
            ValueTypes::Infinity => write!(f, "infinity"),
            ValueTypes::NativeFunction => write!(f, "native_function"),
            ValueTypes::Function => write!(f, "function"),
            ValueTypes::Closure => write!(f, "(function)"),
            ValueTypes::Table => write!(f, "table"),
            ValueTypes::UserData => write!(f, "userdata"),
            #[cfg(feature = "vectors")]
            ValueTypes::Vec3 => write!(f, "vec3"),
            #[cfg(feature = "vectors")]
            ValueTypes::Vec2 => write!(f, "vec2"),
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
