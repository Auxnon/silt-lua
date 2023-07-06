use std::fmt::{self, Display, Formatter};

pub enum OpCode {
    CONSTANT { constant: u8 },
    DEFINE_GLOBAL { constant: u8 },
    GET_GLOBAL { constant: u8 },
    SET_GLOBAL { constant: u8 },
    DEFINE_LOCAL { constant: u8 },
    GET_LOCAL { constant: u8 },
    SET_LOCAL { constant: u8 },
    RETURN,
    POP,
    ADD,
    SUB,
    MULTIPLY,
    DIVIDE,
    NEGATE,
    CONCAT,
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
    PRINT,

    LITERAL { dest: u8, literal: u8 },
}
pub enum Tester {
    CONSTANT(u8),
    RETURN,
}

impl Display for OpCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DEFINE_GLOBAL { constant } => {
                write!(f, "OP_DEFINE_GLOBAL {}", constant)
            }
            Self::GET_GLOBAL { constant } => {
                write!(f, "OP_GET_GLOBAL {}", constant)
            }
            Self::DEFINE_LOCAL { constant } => {
                write!(f, "OP_DEFINE_LOCAL {}", constant)
            }
            Self::RETURN => write!(f, "OP_RETURN"),
            Self::POP => write!(f, "OP_POP"),
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
            Self::CONCAT => write!(f, "OP_CONCAT"),
            Self::LITERAL { dest, literal } => {
                write!(f, "OP_LITERAL {} {}", dest, literal)
            }
            Self::NIL => write!(f, "OP_NIL"),
            Self::TRUE => write!(f, "OP_TRUE"),
            Self::FALSE => write!(f, "OP_FALSE"),
            Self::NOT => write!(f, "OP_NOT"),
            Self::PRINT => write!(f, "OP_PRINT"),

            Self::EQUAL => write!(f, "OP_EQUAL"),
            Self::NOT_EQUAL => write!(f, "OP_NOT_EQUAL"),
            Self::LESS => write!(f, "OP_LESS"),
            Self::LESS_EQUAL => write!(f, "OP_LESS_EQUAL"),
            Self::GREATER => write!(f, "OP_GREATER"),
            Self::GREATER_EQUAL => write!(f, "OP_GREATER_EQUAL"),
            Self::SET_GLOBAL { constant } => {
                write!(f, "OP_SET_GLOBAL {}", constant)
            }
            Self::GET_LOCAL { constant } => {
                write!(f, "OP_GET_LOCAL {}", constant)
            }
            Self::SET_LOCAL { constant } => {
                write!(f, "OP_SET_LOCAL {}", constant)
            }
        }
    }
}
