use std::fmt::{self, Display, Formatter};

pub enum OpCode {
    CONSTANT { constant: u8 },
    RETURN,
    ADD,
    SUB,
    MULTIPLY,
    DIVIDE,
    NEGATE,
    NOT,
    NIL,
    TRUE,
    FALSE,
    EQUAL,
    NOT_EQUAL,
    LESS,
    LESS_EQUAL,
    GREATER,
    GREATER_EQUAL,

    LITERAL { dest: u8, literal: u8 },
}
pub enum Tester {
    CONSTANT(u8),
    RETURN,
}

impl Display for OpCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::RETURN => write!(f, "OP_RETURN"),
            Self::CONSTANT { constant } => {
                write!(f, "OP_CONSTANT {}", constant)
            }
            Self::ADD => {
                write!(f, "OP_ADD")
            }
            Self::SUB => {
                write!(f, "OP_SUBTRACT")
            }
            Self::MULTIPLY => {
                write!(f, "OP_MULTIPLY")
            }
            Self::DIVIDE => {
                write!(f, "OP_DIVIDE")
            }
            Self::NEGATE => write!(f, "OP_NEGATE"),
            Self::LITERAL { dest, literal } => {
                write!(f, "OP_LITERAL {} {}", dest, literal)
            }
            Self::NIL => write!(f, "OP_NIL"),
            Self::TRUE => write!(f, "OP_TRUE"),
            Self::FALSE => write!(f, "OP_FALSE"),
            Self::NOT => write!(f, "OP_NOT"),

            Self::EQUAL => write!(f, "OP_EQUAL"),
            Self::NOT_EQUAL => write!(f, "OP_NOT_EQUAL"),
            Self::LESS => write!(f, "OP_LESS"),
            Self::LESS_EQUAL => write!(f, "OP_LESS_EQUAL"),
            Self::GREATER => write!(f, "OP_GREATER"),
            Self::GREATER_EQUAL => write!(f, "OP_GREATER_EQUAL"),
        }
    }
}
