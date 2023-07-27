use std::fmt::{self, Display, Formatter};

pub enum OpCode {
    CONSTANT { constant: u8 },
    DEFINE_GLOBAL { constant: u8 },
    GET_GLOBAL { constant: u8 },
    SET_GLOBAL { constant: u8 },
    DEFINE_LOCAL { constant: u8 },
    GET_LOCAL { index: u8 },
    SET_LOCAL { index: u8 },
    // TODO this the size bottleneck but we were considering word size anyway soooooo
    // TODO also we could explore a popless goto_if for if statements while conditionals still use the pop variant
    GOTO_IF_FALSE(u16),
    GOTO_IF_TRUE(u16),
    FORWARD(u16),
    REWIND(u16),
    RETURN,
    POP,
    POPN(u8),
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
    META(u8),
    CALL(u8),

    LITERAL { dest: u8, literal: u8 },
}
pub enum Tester {
    CONSTANT(u8),
    RETURN,
}

impl Display for OpCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CALL(i) => write!(f, "call({})", i),
            Self::META(_) => write!(f, "META"),
            Self::GOTO_IF_FALSE(offset) => {
                write!(f, "OP_GOTO_IF_FALSE {}", offset)
            }
            Self::GOTO_IF_TRUE(offset) => {
                write!(f, "OP_GOTO_IF_TRUE {}", offset)
            }
            Self::FORWARD(offset) => write!(f, "OP_FORWARD {}", offset),
            Self::REWIND(offset) => write!(f, "OP_REWIND {}", offset),
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
            Self::POPN(n) => {
                write!(f, "OP_POPx{}", n)
            }
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
            Self::GET_LOCAL { index } => {
                write!(f, "OP_GET_LOCAL {}", index)
            }
            Self::SET_LOCAL { index } => {
                write!(f, "OP_SET_LOCAL {}", index)
            }
        }
    }
}
