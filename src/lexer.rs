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
}

impl Lexer<'_> {
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
        self.iterator.next();
    }
    fn look(&mut self) -> Option<&char> {
        self.iterator.peek()
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            match self.iterator.peek() {
                Some(c) => match c {
                    ' ' | '\r' | '\t' => self.eat(),
                    '\n' => {
                        self.line += 1;
                        self.eat();
                    }

                    'a'..='z' | 'A'..='Z' => {
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
                        return Some(match cc {
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
                    '0'..='9' => {
                        self.start = self.current;
                        self.eat();
                        while self.current < self.end {
                            match self.iterator.peek() {
                                Some(c) => match c {
                                    '0'..='9' => {
                                        self.eat();
                                    }
                                    '.' => {
                                        self.eat();
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
                                println!("{} is not a number", cc);
                                return None;
                            }
                        };
                        return Some(Token::Number(n));
                    }
                    '=' => {
                        self.eat();
                        return Some(match self.iterator.peek() {
                            Some(c) => match c {
                                '=' => {
                                    self.eat();
                                    Token::Equal
                                }
                                _ => Token::Assign,
                            },
                            None => Token::Equal,
                        });
                    }
                    '+' => {
                        self.eat();
                        return Some(match self.look() {
                            Some(c) => match c {
                                '+' => {
                                    self.eat();
                                    Token::Increment
                                }
                                '=' => {
                                    self.eat();
                                    Token::AddAssign
                                }
                                _ => Token::Add,
                            },
                            None => Token::Add,
                        });
                    }
                    '-' => {
                        self.eat();
                        return Some(match self.look() {
                            Some(c) => match c {
                                '-' => {
                                    self.eat();
                                    Token::Decrement
                                }
                                '=' => {
                                    self.eat();
                                    Token::SubAssign
                                }
                                _ => Token::Sub,
                            },
                            None => Token::Sub,
                        });
                    }
                    '(' => {
                        self.eat();
                        return Some(Token::OpenParen);
                    }
                    ')' => {
                        self.eat();
                        return Some(Token::CloseParen);
                    }
                    ';' => {
                        self.eat();
                        return Some(Token::SemiColon);
                    }
                    _ => self.eat(),
                },
                None => return None,
            }
        }
        return None;
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

        // }
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
