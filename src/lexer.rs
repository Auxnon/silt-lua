use crate::{
    error::{ErrorTuple, Location, SiltError},
    token::{Flag, Operator, Token},
};

enum Mode {
    Normal,
    Flag,
    Typer,
    // LookAhead,
}
pub struct Lexer<'c> {
    pub source: &'c str,
    pub iterator: std::iter::Peekable<std::vec::IntoIter<char>>,
    pub start: usize,
    pub end: usize,
    pub current: usize,
    pub line: usize,
    pub column: usize,
    pub column_start: usize,
    mode: Mode,
    // ahead_buffer: Vec<TokenOption>,
}

pub type TokenTuple = (Token, Location);
pub type TokenResult = Result<TokenTuple, ErrorTuple>;
pub type TokenOption = Option<TokenResult>;

impl Iterator for Lexer<'_> {
    type Item = TokenResult;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        match self.mode {
            Mode::Normal => self.step(),
            Mode::Flag => match self.get_flag() {
                Some(r) => Some(r),
                None => {
                    self.mode = Mode::Normal;
                    self.step()
                }
            },
            // Mode::LookAhead => {
            //     if self.ahead_buffer.is_empty() {
            //         self.mode = Mode::Normal;
            //         self.step()
            //     } else {
            //         self.ahead_buffer.remove(0)
            //     }
            // }
            Mode::Typer => self.colon_blow(),
        }
    }
}

impl<'c> Lexer<'c> {
    pub fn new(source: &'c str) -> Self {
        let len = source.len();
        // TODO is this insane?
        let chars = source.chars().collect::<Vec<_>>().into_iter().peekable();
        Lexer {
            source,
            start: 0,
            column: 0,
            column_start: 0,
            current: 0,
            end: len,
            line: 1,
            iterator: chars,
            mode: Mode::Normal,
            // ahead_buffer: Vec::new(),
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

    fn _error(&mut self, code: SiltError) -> TokenResult {
        Err(ErrorTuple {
            code,
            location: (self.line, self.start),
        })
    }

    fn error(&mut self, code: SiltError) -> TokenOption {
        Some(self._error(code))
    }

    // pub fn get_errors(&mut self) -> Vec<ErrorTuple> {
    //     self.error_list.drain(..).collect()
    // }

    fn _send(&mut self, token: Token) -> TokenResult {
        Ok((token, (self.line, self.column_start + 1)))
    }

    fn send(&mut self, token: Token) -> TokenOption {
        // self.tokens.push(token);
        // self.locations.push((self.line, self.column_start + 1)); // add 1 because column_start is 0-indexed
        Some(self._send(token))
    }

    fn eat_send(&mut self, token: Token) -> TokenOption {
        self.eat();
        self.send(token)
    }

    fn eat_eat_send(&mut self, token: Token) -> TokenOption {
        self.eat();
        self.eat();
        self.send(token)
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

    fn number(&mut self, prefix_dot: bool) -> TokenOption {
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
                    #[cfg(feature = "under-number")]
                    '_' => {
                        self.eat();
                        strip = true;
                    }
                    '.' => {
                        if is_float {
                            return self.error(SiltError::InvalidNumber(self.get_sofar()));
                        }
                        is_float = true;
                        self.eat();
                    }
                    'a'..='z' | 'A'..='Z' | '_' => {
                        return self.error(SiltError::InvalidNumber(self.get_sofar()));
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
                    return self.error(SiltError::NotANumber(cc.to_string()));
                }
            };
            self.send(Token::Number(n))
        } else {
            let n = match if strip {
                cc.replace("_", "").parse::<i64>()
            } else {
                cc.parse::<i64>()
            } {
                Ok(n) => n,
                Err(_) => {
                    return self.error(SiltError::NotANumber(cc.to_string()));
                }
            };
            return self.send(Token::Integer(n));
        }
    }

    fn string(&mut self, apos: bool) -> TokenOption {
        // start column at '"' not within string starter
        self.column_start = self.column;
        self.eat();
        self.start = self.current;
        while self.current < self.end {
            match self.peek() {
                Some(c) => match c {
                    '\n' => {
                        return self.error(SiltError::UnterminatedString);
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
                    return self.error(SiltError::UnterminatedString);
                }
            }
        }
        let cc = self.source[self.start..self.current - 1].to_string();
        self.send(Token::StringLiteral(cc.into_boxed_str()))
    }

    fn multi_line_string(&mut self) -> TokenOption {
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
                    return self.error(SiltError::UnterminatedString);
                }
            }
        }
        let cc = self.source[self.start..self.current - 2].to_string();
        self.send(Token::StringLiteral(cc.into_boxed_str()))
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

    fn get_flag(&mut self) -> TokenOption {
        // let mut words = vec![];
        self.mode = Mode::Flag;
        self.set_start();
        while self.current < self.end {
            match self.eat_out() {
                Some(c) => match c {
                    'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => self.eat(),
                    '\n' => {
                        self.new_line();
                        self.mode = Mode::Normal;
                        break;
                    }
                    _ => {
                        let cc = &self.source[self.start..self.current];
                        match cc.to_lowercase().as_str() {
                            "strict" => return self.send(Token::Flag(Flag::Strict)),
                            "local" => return self.send(Token::Flag(Flag::Local)),
                            _ => {}
                        }
                        // words.push(cc.to_string());
                        self.set_start();
                    }
                },
                None => break,
            }
        }
        None
    }

    /** Follow up a colon to determine if an identifer is listed, this may be either a typing or a method determined by context */
    fn colon_blow(&mut self) -> TokenOption {
        while let Some(' ' | '\r' | '\t') = self.peek() {
            self.eat();
        }

        self.set_start();
        self.word_eater();
        self.mode = Mode::Normal;
        if self.start == self.current {
            return self.error(SiltError::ExpectedLocalIdentifier);
        }
        let cc = &self.source[self.start..self.current];
        self.send(Token::ColonIdentifier(cc.into()))
    }

    // fn look_ahead(&mut self, token: Token) -> TokenOption {
    //     match token{
    //         Token::Identifier(_)=>{
    //             match self.step(){
    //                 Some(Token::ArrowFunction)=>{
    //                     self.mode = Mode::LookAhead;
    //                     self.eat();
    //                     self.ahead_buffer.push(self.send(token));
    //                     return self.send(Token::ArrowFunction);
    //                 }
    //             }
    //         }
    //         Token::OpenParen
    //     }else{

    //     }
    //     while self.current < self.end {
    //         match self.step() {
    //             Some(Ok((tuple))) => {
    //                 if let Token::Identifier(_) | Token::Comma= tuple{
    //                     self.look_ahead( Some(Ok(tuple)));
    //                 }else{
    //                     break;
    //                 }
    //                 // continue

    //         }
    //     }
    //     self.send(token)
    // }

    pub fn step(&mut self) -> TokenOption {
        while self.current < self.end {
            let char = match self.peek() {
                Some(c) => *c,
                None => return None,
            };

            self.set_start();
            if let Some(res) = match char {
                '_' | 'a'..='z' | 'A'..='Z' => {
                    self.eat();
                    self.word_eater();
                    let cc = &self.source[self.start..self.current];
                    // TODO is there ever a scenario where a trie is faster then rust's match?
                    self.send(match cc {
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
                        "goto" => Token::Goto,
                        "class" => Token::Class,
                        "sprint" => Token::Print,
                        _ => Token::Identifier(cc.to_string()), //return self.look_ahead(Token::Identifier(Box::new(cc.to_string()))),
                    })
                }
                '0'..='9' => self.number(false),
                '.' => {
                    self.eat();
                    match self.peek() {
                        Some('0'..='9') => self.number(true),
                        Some('.') => self.eat_send(Token::Op(Operator::Concat)),
                        _ => self.send(Token::Dot),
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
                    self.send(t)
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
                    self.send(t)
                }
                '-' => {
                    self.eat();
                    match self.peek() {
                        Some('-') => {
                            self.eat();
                            if let Some('!') = self.peek() {
                                self.eat();
                                self.get_flag()
                            } else {
                                while self.current < self.end {
                                    if let Some('\n') = self.eat_out() {
                                        self.new_line();
                                        break;
                                    }
                                }
                                None
                            }
                        }
                        Some('=') => {
                            self.eat();
                            self.send(Token::SubAssign)
                        }
                        Some('>') => {
                            self.eat();
                            self.send(Token::ArrowFunction)
                        }
                        _ => self.send(Token::Op(Operator::Sub)),
                    }
                }
                '/' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::DivideAssign)
                        }
                        _ => self.send(Token::Op(Operator::Divide)),
                    }
                }
                '*' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::MultiplyAssign)
                        }
                        _ => self.send(Token::Op(Operator::Multiply)),
                    }
                }
                '%' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::ModulusAssign)
                        }
                        _ => self.send(Token::Op(Operator::Modulus)),
                    }
                }
                '(' => {
                    self.eat();
                    self.send(Token::OpenParen)
                }
                ')' => {
                    self.eat();
                    self.send(Token::CloseParen)
                }
                ';' => {
                    self.eat();
                    self.send(Token::SemiColon)
                }
                ',' => {
                    self.eat();
                    self.send(Token::Comma)
                }
                '"' => self.string(false),
                '\'' => self.string(true),
                '<' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::Op(Operator::LessEqual))
                        }
                        _ => self.send(Token::Op(Operator::Less)),
                    }
                }
                '>' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::Op(Operator::GreaterEqual))
                        }
                        _ => self.send(Token::Op(Operator::Greater)),
                    }
                }
                '[' => {
                    self.eat();
                    match self.peek() {
                        Some(c) => match c {
                            '[' => self.multi_line_string(),
                            // '=' => {
                            //     self.eat();
                            //     return Some(Token::OpenBracketAssign);
                            // }
                            _ => self.send(Token::OpenBracket),
                        },
                        None => self.send(Token::OpenBracket),
                    }
                }
                ']' => {
                    self.eat();
                    self.send(Token::CloseBracket)
                }
                '{' => {
                    self.eat();
                    self.send(Token::OpenBrace)
                }
                '}' => {
                    self.eat();
                    self.send(Token::CloseBrace)
                }
                ' ' | '\r' | '\t' => {
                    self.eat();
                    None
                }
                '\n' => {
                    self.new_line();
                    self.eat();
                    None
                }
                '#' => {
                    self.eat();
                    self.send(Token::Op(Operator::Length))
                }
                ':' => {
                    self.eat();
                    match self.peek() {
                        Some(':') => {
                            self.eat();
                            self.send(Token::ColonColon)
                        }
                        #[cfg(feature = "short-declare")]
                        Some('=') => {
                            self.eat();
                            self.send(Token::Op(Operator::ColonEquals))
                        }
                        _ => {
                            self.send(Token::Colon)
                            // trust me on this, it's easier to parse this way
                            // let t = self.colon_blow();
                            // self.add(t);
                        }
                    }
                }
                '~' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::Op(Operator::NotEqual))
                        }
                        _ => self.send(Token::Op(Operator::Tilde)),
                    }
                }
                #[cfg(feature = "bang")]
                '!' => {
                    self.eat();
                    match self.peek() {
                        Some('=') => {
                            self.eat();
                            self.send(Token::Op(Operator::NotEqual))
                        }
                        _ => self.send(Token::Op(Operator::Not)),
                    }
                }
                cw => {
                    let re = self.error(SiltError::UnexpectedCharacter(cw));
                    self.eat();
                    re
                }
            } {
                return Some(res);
            }
        }
        None
    }
}
