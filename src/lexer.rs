use std::vec;

use crate::token::Token;

pub struct Lexer<'a> {
    // pub tokens: Vec<Token>,
    pub source: String,
    pub iterator: std::iter::Peekable<std::str::Chars<'a>>,
    pub start: usize,
    pub end: usize,
    pub current: usize,
    // pub // currentString: String,
    pub line: usize,
    pub column: usize,
    pub error_out: Option<String>,
    pub tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    //     pub fn new(source: &str) -> Self {
    //         let st = source.to_string();
    //         let it = st.chars().peekable();
    //         Lexer {
    //             tokens: vec![],
    //             source: st,
    //             end: source.len(),
    //             iterator: it,
    //             start: 0,
    //             current: 0,
    //             // currentString: String::new(),
    //             line: 1,
    //         }
    //     }
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
    fn error(&mut self, message: String) {
        self.error_out = Some(format!("{} at  {}:{}", message, self.line, self.column));
    }
    pub fn get_error(&mut self) -> Option<String> {
        self.error_out.clone()
    }
    fn add(&mut self, token: Token) {
        self.tokens.push(token);
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
    fn maybe_add(&mut self, token: Option<Token>) {
        if let Some(t) = token {
            self.tokens.push(t);
        }
    }
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
                        self.error(format!("Invalid number {}", self.get_sofar()));
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
                self.error(format!("{} is not a number", cc));
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
                        self.error("Unterminated string".to_string());
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
                    self.error("Unterminated string".to_string());
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
            match self.peek() {
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
                    self.error("Unterminated string".to_string());
                    return None;
                }
            }
        }
        let cc = self.source[self.start..self.current - 2].to_string();
        Some(Token::StringLiteral(cc))
    }

    pub fn parse(&mut self) -> Vec<Token> {
        while self.current < self.end {
            match self.iterator.peek() {
                Some(c) => {
                    // println!("c :{}: {}", c, self.current);

                    match c {
                        '_' | 'a'..='z' | 'A'..='Z' => {
                            self.start = self.current;
                            self.eat();
                            while self.current < self.end {
                                match self.iterator.peek() {
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
                                "else" => (Token::Else),
                                "elseif" => (Token::ElseIf),
                                "end" => (Token::End),
                                "for" => (Token::For),
                                "while" => (Token::While),
                                "function" => (Token::Function),
                                "in" => (Token::In),
                                "local" => (Token::Local),
                                "nil" => (Token::Nil),
                                "not" => (Token::Not),
                                "or" => (Token::Or),
                                "repeat" => (Token::Repeat),
                                "until" => (Token::Until),
                                "return" => (Token::Return),
                                "then" => (Token::Then),
                                "true" => (Token::True),
                                "false" => (Token::False),
                                "and" => (Token::And),
                                "break" => (Token::Break),
                                "do" => (Token::Do),
                                "class" => (Token::Class),
                                _ => (Token::Identifier(cc.to_string())),
                            });
                        }
                        '0'..='9' => self.number(),
                        '.' => match self.peek() {
                            Some('0'..='9') => self.number(),
                            Some('.') => self.eat_eat_add(Token::Concat),
                            _ => self.eat_add(Token::Call),
                        },
                        '=' => {
                            self.eat();
                            let t = match self.peek() {
                                Some('=') => {
                                    self.eat();
                                    Token::Equal
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
                                _ => Token::Add,
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
                                _ => self.add(Token::Sub),
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
                        cw => {
                            let s = format!("Unexpected character {}", cw);
                            self.error(s)
                        }
                    }
                }
                None => break,
            }

            // if self.current>=self.end{
            //     return None;
            // }
            // match self.iterator.nth(self.current){
            //     Some(c)=>{
            //         match c{

            //         }
            //     }
            // }
            // eat white space
            // while self.current<self.end{
            //     match self.iterator.nth(self.current){
            //         Some(c)=>{
            //             match c{
            //                 ' '|'\r'|'\t'=>self.current+=1,
            //                 '\n'=>{
            //                     self.line+=1;
            //                     self.current+=1;
            //                 }
            //                 'a'..='z'|'A'..='Z'|' =>{

            //                 }
            //                 _=>break,
            //             }
            //         }
            //         None=>break,
            //     }
            // }
            // self.start=self.current;
            // if self.current>=self.end{
            //     return None;
            // }
            // let c=self.iterator.nth(self.current);
            // match c{
            //     Some(c)=>{
            //         match c{
            //             t @ 'a'..='z'|'A'..='Z'|'_'=>{

            //             }
            //             '0'..='9'=>{

            //             }
            //             _=>{

            //             }
            //         }
            //     }
            //     None=>{

            //     }
            // }

            // if self.current<self.end{
        }
        self.tokens.drain(..).collect()
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
