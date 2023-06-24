use std::fmt::{self, Display, Formatter};

pub enum OpCode {
    CONSTANT { constant: u8 },
    RETURN,
    ADD,
    SUB,
    MULTIPLY,
    DIVIDE,
    NEGATE,
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
        }
    }
}
