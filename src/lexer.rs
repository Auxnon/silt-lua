use crate::{
    error::{ErrorTuple, Location, SiltError},
    token::{Flag, Operator, Token},
};

pub struct Lexer {
    pub source: String,
    pub iterator: std::iter::Peekable<std::vec::IntoIter<char>>,
    pub start: usize,
    pub end: usize,
    pub current: usize,
    pub line: usize,
    pub column: usize,
    pub column_start: usize,
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
            column_start: 0,
            current: 0,
            end: len,
            line: 1,
            iterator: chars,
            error_list: Vec::new(),
            tokens: Vec::new(),
            locations: Vec::new(),
        }
    }
    fn set_start(&mut self) {
        self.start = self.current;
        self.column_start = self.column;
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
            location: (self.line, self.start),
        });
    }
    pub fn get_errors(&mut self) -> Vec<ErrorTuple> {
        self.error_list.drain(..).collect()
    }
    fn add(&mut self, token: Token) {
        self.tokens.push(token);
        self.locations.push((self.line, self.column_start + 1)); // add 1 because column_start is 0-indexed
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
    fn number(&mut self, prefix_dot: bool) {
        if prefix_dot {
            self.start = self.current - 1;
            self.column_start = self.column - 1;
        } else {
            self.set_start();
        }
        self.eat();
        let mut is_float = prefix_dot;
        let mut strip = false;
        while self.current < self.end {
            match self.peek() {
                Some(c) => match c {
                    '0'..='9' => {
                        self.eat();
                    }
                    '_' => {
                        self.eat();
                        strip = true;
                    }
                    '.' => {
                        if is_float {
                            self.error(SiltError::InvalidNumber(self.get_sofar()));
                            return;
                        }
                        is_float = true;
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
        if is_float {
            let n = match if strip {
                cc.replace("_", "").parse::<f64>()
            } else {
                cc.parse::<f64>()
            } {
                Ok(n) => n,
                Err(_) => {
                    self.error(SiltError::NotANumber(cc.to_string()));
                    return;
                }
            };
            self.add(Token::Number(n));
        } else {
            let n = match if strip {
                cc.replace("_", "").parse::<i64>()
            } else {
                cc.parse::<i64>()
            } {
                Ok(n) => n,
                Err(_) => {
                    self.error(SiltError::NotANumber(cc.to_string()));
                    return;
                }
            };
            self.add(Token::Integer(n));
        }
    }

    fn string(&mut self, apos: bool) -> Option<Token> {
        // start column at '"' not within string starter
        self.column_start = self.column;
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
        Some(Token::StringLiteral(cc.into_boxed_str()))
    }
    fn multi_line_string(&mut self) -> Option<Token> {
        self.column_start = self.column;
        self.eat();
        self.start = self.current;
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
        Some(Token::StringLiteral(cc.into_boxed_str()))
    }
    fn word_eater(&mut self) {
        while self.current < self.end {
            match self.peek() {
                Some(c) => match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => self.eat(),
                    _ => break,
                },
                None => break,
            }
        }
    }
    fn get_flags(&mut self) {
        // let mut words = vec![];
        self.set_start();
        while self.current < self.end {
            match self.eat_out() {
                Some(c) => match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => self.eat(),
                    '\n' => {
                        self.new_line();
                        break;
                    }
                    _ => {
                        let cc = &self.source[self.start..self.current];
                        match cc.to_lowercase().as_str() {
                            "strict" => self.add(Token::Flag(Flag::Strict)),
                            "local" => self.add(Token::Flag(Flag::Local)),
                            _ => {}
                        }
                        // words.push(cc.to_string());
                        self.set_start();
                    }
                },
                None => break,
            }
        }
    }
    /** Follow up a colon to determine if an identifer is listed, this may be either a typing or a method determined by context */
    fn colon_blow(&mut self) -> Token {
        while let Some(' ' | '\r' | '\t') = self.peek() {
            self.eat();
        }
        self.set_start();
        self.word_eater();
        let cc = &self.source[self.start..self.current];
        Token::ColonIdentifier(cc.into())
    }

    pub fn parse(&mut self) -> (Vec<Token>, Vec<Location>) {
        while self.current < self.end {
            let char = match self.peek() {
                Some(c) => *c,
                None => break,
            };

            self.set_start();
            match char {
                '_' | 'a'..='z' | 'A'..='Z' => {
                    self.eat();
                    self.word_eater();
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
                        #[cfg(feature = "global")]
                        "global" => Token::Global,
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
                        "sprint" => Token::Print,
                        _ => Token::Identifier(cc.into()),
                    });
                }
                '0'..='9' => self.number(false),
                '.' => {
                    self.eat();
                    match self.peek() {
                        Some('0'..='9') => self.number(true),
                        Some('.') => self.eat_add(Token::Op(Operator::Concat)),
                        _ => self.add(Token::Call),
                    }
                }
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
                            if let Some('!') = self.peek() {
                                self.eat();
                                self.get_flags();
                            } else {
                                while self.current < self.end {
                                    if let Some('\n') = self.eat_out() {
                                        self.new_line();
                                        break;
                                    }
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
                '<' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::Op(Operator::LessEqual));
                        }
                        _ => self.add(Token::Op(Operator::Less)),
                    };
                }
                '>' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::Op(Operator::GreaterEqual));
                        }
                        _ => self.add(Token::Op(Operator::Greater)),
                    };
                }
                '[' => {
                    self.eat();
                    match self.peek() {
                        Some(c) => match c {
                            '[' => {
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
                '#' => {
                    self.eat();
                    self.add(Token::LengthOp);
                }
                ':' => {
                    self.eat();
                    self.add(Token::Colon);
                    // trust me on this, it's easier to parse this way
                    let t = self.colon_blow();
                    self.add(t);
                }
                '~' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::Op(Operator::NotEqual));
                        }
                        _ => self.add(Token::Op(Operator::Tilde)),
                    };
                }
                #[cfg(feature = "bang")]
                '!' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.add(Token::Op(Operator::NotEqual));
                        }
                        _ => self.add(Token::Op(Operator::Not)),
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
}
