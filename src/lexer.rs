use crate::{
    error::{ErrorTuple, Location, SiltError},
    token::{Operator, Token},
};

pub struct Lexer {
    pub source: String,
    pub iterator: std::iter::Peekable<std::vec::IntoIter<char>>,
    pub start: usize,
    pub end: usize,
    pub current: usize,
    pub line: usize,
    pub column: usize,
    pub error_list: Vec<ErrorTuple>,
    pub tokens: Vec<Token>,
    pub locations: Vec<Location>,
}

impl<'a> Lexer {
    pub fn new(source: String) -> Self {
        let st = source.clone();
        let len = st.len();
        // TODO is this insane?
        let chars = source.chars().collect::<Vec<_>>().into_iter().peekable();
        Lexer {
            source: st,
            start: 0,
            column: 0,
            current: 0,
            end: len,
            line: 1,
            iterator: chars,
            error_list: Vec::new(),
            tokens: Vec::new(),
            locations: Vec::new(),
        }
    }
    fn eat(&mut self) {
        self.current += 1;
        self.column += 1;
        self.iterator.next();
    }
    fn eat_out(&mut self) -> Option<char> {
        self.current += 1;
        self.column += 1;
        self.iterator.next()
    }
    fn peek(&mut self) -> Option<&char> {
        self.iterator.peek()
    }
    fn error(&mut self, code: SiltError) {
        self.error_list.push(ErrorTuple {
            code,
            location: (self.line, self.column),
        });
    }
    pub fn get_errors(&mut self) -> Vec<ErrorTuple> {
        self.error_list.drain(..).collect()
    }
    fn add(&mut self, token: Token) {
        self.tokens.push(token);
        self.locations.push((self.line, self.column));
    }
    fn eat_add(&mut self, token: Token) {
        self.eat();
        self.add(token);
    }
    fn eat_eat_add(&mut self, token: Token) {
        self.eat();
        self.eat();
        self.add(token);
    }
    // fn maybe_add(&mut self, token: Option<Token>) {
    //     if let Some(t) = token {
    //         self.tokens.push(t);
    //         self.locations.push((self.line, self.column));
    //     }
    // }
    fn new_line(&mut self) {
        self.line += 1;
        self.column = 0;
    }
    fn get_sofar(&self) -> String {
        self.source[self.start..self.current].to_string()
    }
    fn number(&mut self) {
        self.start = self.current;
        self.eat();
        while self.current < self.end {
            match self.peek() {
                Some(c) => match c {
                    '0'..='9' => {
                        self.eat();
                    }
                    '.' => {
                        self.eat();
                    }
                    'a'..='z' | 'A'..='Z' | '_' => {
                        self.error(SiltError::InvalidNumber(self.get_sofar()));
                        return;
                    }
                    _ => break,
                },
                None => break,
            }
        }
        let cc = &self.source[self.start..self.current];
        let n = match cc.parse::<f64>() {
            Ok(n) => n,
            Err(_) => {
                self.error(SiltError::NotANumber(cc.to_string()));
                return;
            }
        };
        self.add(Token::Number(n));
    }

    fn string(&mut self, apos: bool) -> Option<Token> {
        self.eat();
        self.start = self.current;
        while self.current < self.end {
            match self.peek() {
                Some(c) => match c {
                    '\n' => {
                        self.error(SiltError::UnterminatedString);
                        return None;
                    }
                    '\'' => {
                        self.eat();
                        if apos {
                            break;
                        }
                    }
                    '"' => {
                        self.eat();
                        if !apos {
                            break;
                        }
                    }
                    _ => {
                        self.eat();
                    }
                },
                None => {
                    self.error(SiltError::UnterminatedString);
                    return None;
                }
            }
        }
        let cc = self.source[self.start..self.current - 1].to_string();
        Some(Token::StringLiteral(cc))
    }
    fn multi_line_string(&mut self) -> Option<Token> {
        self.start = self.current;
        self.eat();
        while self.current < self.end {
            let char = self.peek();
            match char {
                Some(c) => match c {
                    '\n' => {
                        self.new_line();
                        self.eat();
                    }
                    ']' => {
                        self.eat();
                        if self.peek() == Some(&']') {
                            self.eat();
                            break;
                        }
                    }
                    _ => {
                        self.eat();
                    }
                },
                None => {
                    self.error(SiltError::UnterminatedString);
                    return None;
                }
            }
        }
        let cc = self.source[self.start..self.current - 2].to_string();
        Some(Token::StringLiteral(cc))
    }

    pub fn parse(&mut self) -> (Vec<Token>, Vec<Location>) {
        while self.current < self.end {
            let char = match self.peek() {
                Some(c) => *c,
                None => break,
            };

            match char {
                '_' | 'a'..='z' | 'A'..='Z' => {
                    self.start = self.current;
                    self.eat();
                    while self.current < self.end {
                        match self.peek() {
                            Some(c) => match c {
                                'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => self.eat(),
                                _ => break,
                            },
                            None => break,
                        }
                    }
                    let cc = &self.source[self.start..self.current];
                    self.add(match cc {
                        "if" => Token::If,
                        "else" => Token::Else,
                        "elseif" => Token::ElseIf,
                        "end" => Token::End,
                        "for" => Token::For,
                        "while" => Token::While,
                        "function" => Token::Function,
                        "in" => Token::In,
                        "local" => Token::Local,
                        "nil" => Token::Nil,
                        "not" => Token::Op(Operator::Not),
                        "or" => Token::Op(Operator::Or),
                        "repeat" => Token::Repeat,
                        "until" => Token::Until,
                        "return" => Token::Return,
                        "then" => Token::Then,
                        "true" => Token::True,
                        "false" => Token::False,
                        "and" => Token::Op(Operator::And),
                        "break" => Token::Break,
                        "do" => Token::Do,
                        "class" => Token::Class,
                        "print" => Token::Print,
                        _ => Token::Identifier(cc.to_string()),
                    });
                }
                '0'..='9' => self.number(),
                '.' => match self.peek() {
                    Some('0'..='9') => self.number(),
                    Some('.') => self.eat_eat_add(Token::Op(Operator::Concat)),
                    _ => self.eat_add(Token::Call),
                },
                '=' => {
                    self.eat();
                    let t = match self.peek() {
                        Some('=') => {
                            self.eat();
                            Token::Op(Operator::Equal)
                        }
                        _ => Token::Assign,
                    };
                    self.add(t);
                }
                '+' => {
                    self.eat();
                    let t = match self.peek() {
                        Some('=') => {
                            self.eat();
                            Token::AddAssign
                        }
                        _ => Token::Op(Operator::Add),
                    };
                    self.add(t);
                }
                '-' => {
                    self.eat();
                    match self.peek() {
                        Some('-') => {
                            self.eat();
                            while self.current < self.end {
                                if let Some('\n') = self.eat_out() {
                                    self.new_line();
                                    break;
                                }
                            }
                        }
                        Some('=') => {
                            self.eat();
                            self.add(Token::SubAssign);
                        }
                        Some('>') => {
                            self.eat();
                            self.add(Token::ArrowFunction)
                        }
                        _ => self.add(Token::Op(Operator::Sub)),
                    };
                }
                '/' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::DivideAssign);
                        }
                        _ => self.add(Token::Op(Operator::Divide)),
                    };
                }
                '*' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::MultiplyAssign);
                        }
                        _ => self.add(Token::Op(Operator::Multiply)),
                    };
                }
                '%' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::ModulusAssign);
                        }
                        _ => self.add(Token::Op(Operator::Modulus)),
                    };
                }
                '(' => {
                    self.eat();
                    self.add(Token::OpenParen);
                }
                ')' => {
                    self.eat();
                    self.add(Token::CloseParen);
                }
                ';' => {
                    self.eat();
                    self.add(Token::SemiColon);
                }
                ',' => {
                    self.eat();
                    self.add(Token::Comma);
                }
                '"' => {
                    if let Some(t) = self.string(false) {
                        self.add(t);
                    }
                }
                '\'' => {
                    if let Some(t) = self.string(true) {
                        self.add(t);
                    }
                }
                '[' => {
                    self.eat();
                    match self.peek() {
                        Some(c) => match c {
                            '[' => {
                                self.eat();
                                if let Some(t) = self.multi_line_string() {
                                    self.add(t);
                                }
                            }
                            // '=' => {
                            //     self.eat();
                            //     return Some(Token::OpenBracketAssign);
                            // }
                            _ => self.add(Token::OpenBracket),
                        },
                        None => self.add(Token::OpenBracket),
                    }
                }
                ']' => {
                    self.eat();
                    self.add(Token::CloseBracket);
                }
                ' ' | '\r' | '\t' => self.eat(),
                '\n' => {
                    self.new_line();
                    self.eat();
                }
                #[cfg(feature = "bang")]
                '!' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::NotEqual);
                        }
                        _ => self.add(Token::Not),
                    };
                }
                cw => {
                    self.error(SiltError::UnexpectedCharacter(cw));
                    self.eat();
                }
            }
        }

        (
            self.tokens.drain(..).collect(),
            self.locations.drain(..).collect(),
        )
    }
    // fn add_char(&mut self, c: char) {
    //     self.current += 1;
    // }

    // pub fn parse(s:&str){
    //     let tokens: Vec<Token> =vec![];
    //     s.chars().for_each(|c|{
    //         match c{
    //             'a'..='z'|'A'..='Z'|'_'=>{

    //             }
    //             '0'..='9'=>{

    //             }
    //             _=>{

    //             }
    //         }
    //     });
    //     match s{
    //         "if"=>
    //     }
}
