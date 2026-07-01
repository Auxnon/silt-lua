use crate::{token::Token, userdata::MetaMethod};

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
    InvalidVarArgParam,
    InvalidVarArgUsage,
    InvalidVarArgAssignment,
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
    CoerceInt,

    // Userdata errors
    UDNoInitField,
    UDNoInitMethod,
    UDBadCall,
    UDBadCast,
    UDNoMap,
    UDNoFieldGet,
    UDNoFieldSet,
    UDNoMethodRef,
    UDTypeMismatch,
    UDRefDropped,

    //vm
    VmCompileError,
    VmRuntimeError,
    VmCorruptConstant,
    VmUpvalueResolveError,
    VmNonTableOperations(ValueTypes),
    VmValBadConvert(ValueTypes),
    VmNativeParameterMismatch,

    Unknown,

    // Generic custom enums
    Custom(String),
    Network(String),
    IO(String),
}

impl std::error::Error for SiltError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // SiltError::InvalidNumber(s) => Some(s),
            _ => None,
        }
    }
}

// impl From<SiltError> for Box<dyn std::error::Error> {
//     fn from(err: SiltError) -> Self {
//         Box::new(err)
//     }
// }

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

#[derive(Clone)]
pub struct TokenTriple {
    pub line: usize,
    pub col: usize,
    pub index: usize,
    pub length: usize,
}

impl TokenTriple {
    pub fn new(line: usize, col: usize, index: usize, length: usize) -> Self {
        TokenTriple {
            line,
            col,
            index,
            length,
        }
    }
}

impl Default for TokenTriple {
    fn default() -> Self {
        TokenTriple {
            line: 0,
            col: 0,
            index: 0,
            length: 0,
        }
    }
}
pub type TokenCell = (usize, usize);

// impl TripleLocation{
//     pub fn into(&self)-> Location{
//         (self.0,self.2)
//     }
// }

// impl Into<Location> for TripleLocation {
//     fn into(self) -> Location {
//         (self.0,self.2)
//     }
// }
// //     fn from(value: TripleLocation) -> Self {
// //         (value.0, value.2)
// //     }
// // }

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
            Self::InvalidVarArgParam => write!(f, "vararg is not last param in function"),
            Self::InvalidVarArgUsage => write!(f, "vararg used outside of vararg function"),
            Self::InvalidVarArgAssignment => write!(f, "cannot assign to a vararg"),
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
                    "Cannot perform table operations on a non-table value ({})",
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
            Self::VmValBadConvert(t) => write!(f, "Impossible to convert from \"{}\"", t),
            Self::VmNativeParameterMismatch => {
                write!(f, "Cannot call native function with available parameters")
            }

            Self::Unknown => write!(f, "Unknown error"),
            SiltError::MetaMethodMissing(meta_method) => {
                write!(f, "Meta method missing for '{}'", meta_method)
            }
            SiltError::MetaMethodNotCallable(meta_method) => {
                write!(f, "Value for meta method '{}' is not callable", meta_method)
            }
            SiltError::CoerceInt => {
                write!(f, "Value can't be strictly coerced to an integer")
            }
            SiltError::UDNoInitField => write!(f, "UserData field not setup"),
            SiltError::UDNoInitMethod => write!(f, "UserData method not setup"),
            SiltError::UDNoMap => write!(f, "UserData map not setup"),
            SiltError::UDNoFieldGet => write!(f, "UserData field getter does not exist"),
            SiltError::UDNoFieldSet => write!(f, "UserData field setter does not exist"),
            SiltError::UDNoMethodRef => write!(f, "UserData method does not exist"),
            SiltError::UDTypeMismatch => {
                write!(f, "UserData type mismatch during method or field access")
            }
            SiltError::UDRefDropped => {
                write!(f, "UserData weak reference dropped")
            }
            SiltError::UDBadCall => {
                write!(f, "UserData method called with non-userdata self, try :")
            }
            SiltError::UDBadCast => write!(f, "UserData bad downcast"),
            SiltError::Custom(s) => write!(f, "{}", s),
            SiltError::Network(s) => write!(f, "Network Error; {}", s),
            SiltError::IO(s) => write!(f, "Input Output Error; {}", s),
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
    pub location: TokenCell,
}

impl Default for ErrorTuple {
    fn default() -> Self {
        Self {
            code: SiltError::Unknown,
            location: (0, 0),
        }
    }
}
impl Default for &ErrorTuple {
    fn default() -> Self {
        &ErrorTuple {
            code: SiltError::Unknown,
            location: (0, 0),
        }
    }
}

#[derive(Clone)]
pub struct ErrorOut {
    pub errors: Vec<ErrorTuple>,
    pub source: Option<String>,
}

impl ToString for ErrorOut {
    fn to_string(&self) -> String {
        let source = self.source.clone().unwrap_or("unknown".to_string());
        if self.errors.len() > 1 {
            let failed = self
                .errors
                .iter()
                .enumerate()
                .map(|(i, item)| format!("{}. {}", i + 1, item.to_string()))
                .collect::<Vec<_>>()
                .join("\n");
            format!("source [{}] failed with:\n{}", source, failed)
        } else {
            format!(
                "source [{}] failed with: {}",
                source,
                self.errors.first().unwrap_or_default()
            )
        }
    }
}

impl ErrorOut {
    pub fn get_first(&self) -> SiltError {
        let f = self.errors.first();
        match f {
            Some(e) => e.code.clone(),
            None => SiltError::Unknown,
        }
    }

    /// Render every error as a message + source snippet, given the original source
    /// string. `ErrorOut.source` holds the chunk *name*, not the code, so the caller
    /// must supply the source — which is the point: the VM may have run elsewhere.
    pub fn snippet(&self, source: &str) -> String {
        self.errors
            .iter()
            .map(|e| e.snippet(source))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl ErrorTuple {
    /// This error's message plus a source snippet with a caret under its location.
    pub fn snippet(&self, source: &str) -> String {
        format!(
            "error: {} (line {}, col {})\n{}",
            self.code, self.location.0, self.location.1,
            error_snippet(source, self.location)
        )
    }
}

/// Render a one-line source snippet pointing at `location` (1-indexed line, col): the
/// offending line with a caret beneath the column.
///
/// This is a standalone helper so a caller that still holds the source can produce a
/// snippet for an error raised elsewhere — e.g. a VM running on another thread that no
/// longer has the source. It degrades gracefully when the location is out of range.
///
/// ```text
///  3 | local c = )
///    |           ^
/// ```
pub fn error_snippet(source: &str, location: TokenCell) -> String {
    let (line, col) = location;
    let text = match line.checked_sub(1).and_then(|i| source.lines().nth(i)) {
        Some(t) => t,
        None => return format!("  (line {} not in source)", line),
    };
    let gutter = line.to_string();
    let pad = " ".repeat(gutter.len());
    // Echo the line's leading characters as the caret indent (tabs stay tabs) so the
    // caret aligns under tab- or space-indented code.
    let indent: String = text
        .chars()
        .take(col.saturating_sub(1))
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    format!("{} | {}\n{} | {}^", gutter, text, pad, indent)
}

impl From<Vec<ErrorTuple>> for SiltError {
    fn from(value: Vec<ErrorTuple>) -> Self {
        value.into_iter().next().unwrap().code
    }
}

impl std::fmt::Display for ErrorTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.code, self.location.0, self.location.1)
    }
}
