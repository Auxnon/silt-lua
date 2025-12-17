use core::fmt::Display;
use std::fmt::write;
// implement clone

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // TODO temporary
    Print,

    Identifier(String),
    Break,
    Do,
    If,
    Else,
    ElseIf,
    End,
    For,
    While,
    Function,
    ArrowFunction,
    In,
    Local,
    Goto,

    Repeat,
    Until,
    Return,
    Then,

    // symbols
    Dot,
    Assign, //Equal

    // operator
    Op(Operator),
    // Not,
    // And,
    // Or,
    // Add,
    // Sub,
    // Multiply,
    // Divide,
    // FloorDivide,
    // Modulus,
    // Exponent,
    // Concat,
    // Equal,
    // NotEqual,
    // LessThan,
    // LessThanOrEqual,
    // GreaterThan,
    // GreaterThanOrEqual,
    AddAssign,
    SubAssign,
    MultiplyAssign,
    DivideAssign,
    ModulusAssign,
    Colon,                     //:
    ColonColon,                //::
    ColonIdentifier(Box<str>), // :ident (for types and calls)
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    SemiColon,
    Comma,

    // primary
    Nil,
    Number(f64),
    Integer(i64),
    StringLiteral(Box<str>),
    True,
    False,

    Comment,
    EOF,

    //extra
    // Bang, // !
    Class,
    Global,
    Flag(Flag),
    Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Not,
    Tilde,
    And,
    Or,
    Add,
    Sub,
    Multiply,
    Divide,
    FloorDivide,
    Modulus,
    Exponent,
    Concat,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Length,
    ColonEquals,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Flag {
    Strict,
    Local,
}
impl Token {
    pub fn unwrap_identifier(&self) -> &String {
        match self {
            Token::Identifier(s) => s,
            _ => panic!("unwrap_identifier"),
        }
    }
}
impl Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Flag::Strict => write!(f, "strict"),
            Flag::Local => write!(f, "local"),
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Token::Print => write!(f, "print"),
            Token::Op(ref op) => write!(f, "{}", op),
            Token::Break => write!(f, "break"),
            Token::Do => write!(f, "do"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::ElseIf => write!(f, "elseif"),
            Token::End => write!(f, "end"),
            Token::For => write!(f, "for"),
            Token::While => write!(f, "while"),
            Token::Function => write!(f, "function"),
            Token::ArrowFunction => write!(f, "arrow function"),
            Token::In => write!(f, "in"),
            Token::Local => write!(f, "local"),
            Token::Nil => write!(f, "nil"),
            Token::Repeat => write!(f, "repeat"),
            Token::Until => write!(f, "until"),
            Token::Return => write!(f, "return"),
            Token::Then => write!(f, "then"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Goto => write!(f, "goto"),
            Token::Assign => write!(f, "::="),
            Token::AddAssign => write!(f, "+="),
            Token::SubAssign => write!(f, "-="),
            Token::MultiplyAssign => write!(f, "*="),
            Token::DivideAssign => write!(f, "/="),
            Token::ModulusAssign => write!(f, "%="),
            Token::Colon => write!(f, ":"),
            Token::ColonColon => write!(f, "::"),
            Token::Class => write!(f, "class"),
            Token::Number(n) => write!(f, "{}f", n),
            Token::Integer(i) => write!(f, "{}i", i),
            Token::Identifier(ref s) => write!(f, "ident({})", s),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::OpenBrace => write!(f, "{{"),
            Token::CloseBrace => write!(f, "}}"),
            Token::SemiColon => write!(f, ";"),
            Token::Comma => write!(f, ","),
            Token::StringLiteral(ref s) => write!(f, "string({})", s),
            Token::OpenBracket => write!(f, "["),
            Token::CloseBracket => write!(f, "]"),
            // Token::EOF => write!(f, "EOF"),
            Token::Dot => write!(f, "call"),
            // Token::Bang => write!(f, "!"),
            Token::Type => write!(f, "type"),
            Token::ColonIdentifier(ref ident) => write!(f, ":{}", ident),
            Token::Global => write!(f, "global"),
            Token::Flag(ref flag) => write!(f, "flag({})", flag),
            Self::Comment=> write!(f, "--"),
            Self::EOF => write!(f, "EOF"),
        }
    }
}

impl Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Operator::And => write!(f, "and"),
            Operator::Not => write!(f, "not"),
            Operator::Or => write!(f, "or"),
            Operator::Add => write!(f, "+"),
            Operator::Sub => write!(f, "-"),
            Operator::Multiply => write!(f, "*"),
            Operator::Divide => write!(f, "/"),
            Operator::FloorDivide => write!(f, "//"),
            Operator::Modulus => write!(f, "%"),
            Operator::Exponent => write!(f, "^"),
            Operator::Concat => write!(f, ".."),
            Operator::Equal => write!(f, "=="),
            Operator::NotEqual => write!(f, "~="),
            Operator::Less => write!(f, "<"),
            Operator::LessEqual => write!(f, "<="),
            Operator::Greater => write!(f, ">"),
            Operator::GreaterEqual => write!(f, ">="),
            Operator::Tilde => write!(f, "~"),
            Operator::Length => write!(f, "#"),
            Operator::ColonEquals => write!(f, ":="),
        }
    }
}
