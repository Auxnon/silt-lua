use core::fmt::Display;

pub enum Token {
    Identifier(String),
    And,
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
    Nil,
    Not,
    Or,
    Repeat,
    Until,
    Return,
    Then,
    True,
    False,

    // symbols
    Call,
    Assign, //Equal
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Multiply,
    MultiplyAssign,
    Divide,
    DivideAssign,
    Modulus,
    Exponent,
    LengthOp, //#
    TypeOp,   //:
    Concat,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    SemiColon,
    Comma,

    Equal,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    //extra
    Class,
    Number(f64),
    StringLiteral(String),
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Token::And => write!(f, "and"),
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
            Token::Not => write!(f, "not"),
            Token::Or => write!(f, "or"),
            Token::Repeat => write!(f, "repeat"),
            Token::Until => write!(f, "until"),
            Token::Return => write!(f, "return"),
            Token::Then => write!(f, "then"),
            Token::True => write!(f, "true"),
            Token::False => write!(f, "false"),
            Token::Assign => write!(f, "::="),
            Token::Add => write!(f, "+"),
            Token::AddAssign => write!(f, "+="),
            Token::Sub => write!(f, "-"),
            Token::SubAssign => write!(f, "-="),
            Token::Multiply => write!(f, "*"),
            Token::MultiplyAssign => write!(f, "*="),
            Token::Divide => write!(f, "/"),
            Token::DivideAssign => write!(f, "/="),
            Token::Modulus => write!(f, "%"),
            Token::Exponent => write!(f, "^"),
            Token::LengthOp => write!(f, "#"),
            Token::TypeOp => write!(f, ":"),
            Token::Concat => write!(f, ".."),
            Token::Equal => write!(f, "=="),
            Token::LessThan => write!(f, "<"),
            Token::LessThanOrEqual => write!(f, "<="),
            Token::GreaterThan => write!(f, ">"),
            Token::GreaterThanOrEqual => write!(f, ">="),
            Token::Class => write!(f, "class"),
            Token::Number(n) => write!(f, "f64({})", n),
            Token::Identifier(ref s) => write!(f, "ident({})", s),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::SemiColon => write!(f, ";"),
            Token::Comma => write!(f, ","),
            Token::StringLiteral(ref s) => write!(f, "string({})", s),
            Token::OpenBracket => write!(f, "["),
            Token::CloseBracket => write!(f, "]"),
        }
    }
}
