use crate::{
    error::{ErrorTuple, Location, SiltError},
    token::Token,
};

pub enum Expression {
    BinaryExpression {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    Unary {
        operator: Token,
        right: Box<Expression>,
    },
    Primary {
        literal: Token,
    },
    LiteralExpression {
        value: Token,
    },
    GroupingExpression {
        expression: Box<Expression>,
    },
    VariableExpression {
        name: Token,
    },
    AssignmentExpression {
        name: Token,
        value: Box<Expression>,
    },
    LogicalExpression {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
    },
    CallExpression {
        callee: Box<Expression>,
        arguments: Vec<Expression>,
    },
    GetExpression {
        object: Box<Expression>,
        name: Token,
    },
    SetExpression {
        object: Box<Expression>,
        name: Token,
        value: Box<Expression>,
    },
    ThisExpression {
        keyword: Token,
    },
    SuperExpression {
        keyword: Token,
        method: Token,
    },
    EndOfFile,
}
impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expression::BinaryExpression {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::Unary { operator, right } => write!(f, "({} {})", operator, right),
            Expression::Primary { literal } => write!(f, " {} ", literal),
            Expression::LiteralExpression { value } => write!(f, " {} ", value),
            Expression::GroupingExpression { expression } => write!(f, "G({})", expression),
            Expression::VariableExpression { name } => write!(f, "{}", name),
            Expression::AssignmentExpression { name, value } => {
                write!(f, "({} := {})", name, value)
            }
            Expression::LogicalExpression {
                left,
                operator,
                right,
            } => write!(f, "({} {} {})", operator, left, right),
            Expression::CallExpression { callee, arguments } => {
                let mut s = format!("({}(", callee);
                for arg in arguments {
                    s.push_str(&format!("{},", arg));
                }
                s.push_str("))");
                write!(f, "{}", s)
            }
            Expression::GetExpression { object, name } => write!(f, "({}.{})", object, name),
            Expression::SetExpression {
                object,
                name,
                value,
            } => write!(f, "({}.{}={})", object, name, value),
            Expression::ThisExpression { keyword } => write!(f, "{}", keyword),
            Expression::SuperExpression { keyword, method } => write!(f, "{}.{}", keyword, method),
            Expression::EndOfFile => write!(f, ""),
        }
    }
}
pub struct Parser {
    pub iterator: std::iter::Peekable<std::vec::IntoIter<Token>>,
    pub locations: Vec<Location>,
    pub current: usize,
    pub errors: Vec<ErrorTuple>,
    pub valid: bool,
}
impl Parser {
    pub fn new(t: Vec<Token>, p: Vec<Location>) -> Parser {
        let ee = t.into_iter().peekable();
        // let tt = t.iter().peekable();
        Parser {
            iterator: ee,
            locations: p,
            current: 0,
            errors: vec![],
            valid: true,
        }
    }
    // pub fn advance(&mut self) -> Option<Token> {
    //     self.current += 1;
    //     self.iterator.next().cloned()
    // }
    fn error(&mut self, code: SiltError) {
        self.valid = false;
        self.errors.push(ErrorTuple {
            code,
            location: self.locations[self.current],
        });
    }
    pub fn eat(&mut self) {
        self.current += 1;
        println!(
            "{}",
            match self.iterator.next() {
                Some(t) => format!("{}", t),
                None => format!("None"),
            }
        );
    }
    /** only use after peek */
    pub fn eat_out(&mut self) -> Token {
        self.current += 1;
        self.iterator.next().unwrap()
    }
    fn peek(&mut self) -> Option<&Token> {
        self.iterator.peek()
    }
    pub fn parse(&mut self) -> Expression {
        self.expression()
    }
    // op unary or primary
    pub fn unary(&mut self) -> Expression {
        if let Some(&Token::Sub) | Some(&Token::Not) = self.peek() {
            let operator = self.eat_out();
            let right = self.unary();
            Expression::Unary {
                operator,
                right: Box::new(right),
            }
        } else {
            self.primary()
        }
    }
    pub fn expression(&mut self) -> Expression {
        self.equality()
    }
    pub fn equality(&mut self) -> Expression {
        let mut exp = self.comparison();
        while let Some(&Token::NotEqual) | Some(&Token::Equal) = self.peek() {
            let operator = self.eat_out();
            let right = self.comparison();
            exp = Expression::BinaryExpression {
                left: Box::new(exp),
                operator,
                right: Box::new(right),
            };
        }
        exp
    }
    fn comparison(&mut self) -> Expression {
        let mut exp = self.term();

        while let Some(&Token::LessThan)
        | Some(&Token::LessThanOrEqual)
        | Some(&Token::GreaterThan)
        | Some(&Token::GreaterThanOrEqual) = self.peek()
        {
            let operator = self.eat_out();
            let right = self.term();
            exp = Expression::BinaryExpression {
                left: Box::new(exp),
                operator: operator,
                right: Box::new(right),
            };
        }
        exp
    }
    fn term(&mut self) -> Expression {
        let mut exp = self.factor();
        while let Some(&Token::Add) | Some(&Token::Sub) = self.peek() {
            let operator = self.eat_out();
            let right = self.factor();
            exp = Expression::BinaryExpression {
                left: Box::new(exp),
                operator: operator,
                right: Box::new(right),
            };
        }
        exp
    }
    fn factor(&mut self) -> Expression {
        let mut exp = self.unary();
        while let Some(&Token::Multiply) | Some(&Token::Divide) | Some(&Token::Modulus) =
            self.peek()
        {
            let operator = self.eat_out();
            let right = self.unary();
            exp = Expression::BinaryExpression {
                left: Box::new(exp),
                operator: operator,
                right: Box::new(right),
            };
        }
        exp
    }
    fn primary(&mut self) -> Expression {
        // let err
        match self.peek() {
            Some(&Token::Number(_)) => Expression::LiteralExpression {
                value: self.eat_out(),
            },
            Some(&Token::True) | Some(&Token::False) | Some(&Token::Nil) => {
                Expression::LiteralExpression {
                    value: self.eat_out(),
                }
            }
            Some(&Token::OpenParen) => {
                self.eat();
                let exp = self.expression();
                if let Token::CloseParen = self.eat_out() {
                } else {
                    self.error(SiltError::UnterminatedParenthesis);
                }

                Expression::GroupingExpression {
                    expression: Box::new(exp),
                }
            }
            Some(&Token::Identifier(_)) => Expression::VariableExpression {
                name: self.eat_out(),
            },
            Some(&Token::EOF) => Expression::EndOfFile,
            Some(d) => {
                // TODO peek is mut!
                // self.error(SiltError::InvalidExpressionToken(d.clone()));
                Expression::LiteralExpression { value: Token::Nil }
            }
            _ => {
                self.error(SiltError::EarlyEndOfFile);
                Expression::EndOfFile
            }
        }
    }
}
