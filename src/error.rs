use crate::token::Token;

pub enum SiltError {
    InvalidNumber(String),
    NotANumber(String),
    UnexpectedCharacter(char),
    UnterminatedString,

    UnterminatedParenthesis,
    InvalidExpressionToken(Token),
    EarlyEndOfFile,
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
            SiltError::InvalidExpressionToken(t) => write!(f, "Invalid expression token: {}", t),
            SiltError::EarlyEndOfFile => write!(f, "File ended early"),
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
