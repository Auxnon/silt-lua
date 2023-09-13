use std::fmt::{self, Display, Formatter};

#[allow(non_camel_case_types)]
#[derive(Clone)]
pub enum OpCode {
    CONSTANT {
        constant: u8,
    },
    CLOSURE {
        constant: u8,
    },
    DEFINE_GLOBAL {
        constant: u8,
    },
    GET_GLOBAL {
        constant: u8,
    },
    SET_GLOBAL {
        constant: u8,
    },
    DEFINE_LOCAL {
        constant: u8,
    },
    GET_LOCAL {
        index: u8,
    },
    SET_LOCAL {
        index: u8,
    },
    GET_UPVALUE {
        index: u8,
    },
    SET_UPVALUE {
        index: u8,
    },
    // TODO this the size bottleneck but we were considering word size anyway soooooo
    // TODO also we could explore a popless goto_if for if statements while conditionals still use the pop variant
    GOTO_IF_FALSE(u16),
    GOTO_IF_TRUE(u16),
    // TODO this should replace normal goto_if_false
    POP_AND_GOTO_IF_FALSE(u16),
    /** Compares 2nd(end) and 3rd(iter) value on stack, if greater then forward by X, otherwise push 3rd(iter) on to new stack  */
    FOR_NUMERIC(u16),
    FORWARD(u16),
    REWIND(u16),
    RETURN,
    POP,
    POPS(u8),
    CLOSE_UPVALUES(u8),
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
    REGISTER_UPVALUE {
        index: u8,
        neighboring: bool,
    },

    LITERAL {
        dest: u8,
        literal: u8,
    },
    LENGTH,
    NEW_TABLE,
    TABLE_INSERT {
        offset: u8,
    },
    TABLE_BUILD(u8),
    TABLE_GET {
        depth: u8,
    },
    TABLE_GET_BY_CONSTANT {
        constant: u8,
    },
    TABLE_GET_FROM {
        index: u8,
    },
    /** depth indicates how many index contansts are on the stack. Statement does not need pop */
    TABLE_SET {
        depth: u8,
    },
    // TABLE_SET_BY_CONSTANT {
    //     constant: u8,
    // },
    /** Increment a local value at index with top of stack*/
    INCREMENT {
        index: u8,
    },
}
pub enum Tester {
    CONSTANT(u8),
    RETURN,
}

impl Display for OpCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CALL(i) => write!(f, "OP_CALL({})", i),
            Self::REGISTER_UPVALUE {
                index: i,
                neighboring: n,
            } => write!(f, "OP_REG_UPVALUE {}", i),
            Self::GET_UPVALUE { index: i } => {
                write!(f, "OP_GET_UPVALUE {}", i)
            }
            Self::SET_UPVALUE { index: i } => {
                write!(f, "OP_SET_UPVALUE {}", i)
            }
            Self::META(_) => write!(f, "META"),
            Self::GOTO_IF_FALSE(offset) => {
                write!(f, "OP_GOTO_IF_FALSE {}", offset)
            }
            Self::POP_AND_GOTO_IF_FALSE(offset) => {
                write!(f, "OP_POP_AND_GOTO_IF_FALSE {}", offset)
            }
            Self::GOTO_IF_TRUE(offset) => {
                write!(f, "OP_GOTO_IF_TRUE {}", offset)
            }
            Self::FORWARD(offset) => write!(f, "OP_FORWARD {}", offset),
            Self::REWIND(offset) => write!(f, "OP_REWIND {}", offset),
            Self::FOR_NUMERIC(offset) => {
                write!(f, "OP_FOR_NUMERIC {}", offset)
            }
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
            Self::POPS(n) => {
                write!(f, "OP_POPx{}", n)
            }
            Self::CLOSE_UPVALUES(n) => {
                write!(f, "OP_CLOSE_UPVALUEx{}", n)
            }
            Self::CONSTANT { constant } => {
                write!(f, "OP_CONSTANT {}", constant)
            }
            Self::CLOSURE { constant } => {
                write!(f, "OP_CLOSURE {}", constant)
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
            Self::LENGTH => write!(f, "OP_LENGTH"),
            Self::NEW_TABLE => write!(f, "OP_NEW_TABLE"),
            Self::TABLE_INSERT { offset } => write!(f, "OP_TABLE_INSERT @{}", offset),
            Self::TABLE_BUILD(u) => write!(f, "OP_TABLE_BUILD [;{}]", u),
            Self::TABLE_GET { depth } => write!(f, "OP_TABLE_GET {}[]", depth),
            Self::TABLE_GET_BY_CONSTANT { constant } => {
                write!(f, "OP_TABLE_GET_BY_CONSTANT {}", constant)
            }
            Self::TABLE_GET_FROM { index } => {
                write!(f, "OP_TABLE_GET_FROM {}", index)
            }
            Self::TABLE_SET { depth } => write!(f, "OP_TABLE_SET {}[]", depth),
            // Self::TABLE_SET_BY_CONSTANT { constant } => {
            //     write!(f, "OP_TABLE_SET_BY_CONSTANT {}", constant)
            // }
            Self::INCREMENT { index } => write!(f, "OP_INCREMENT {}", index),
        }
    }
}
