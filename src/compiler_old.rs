use std::iter::{Enumerate, Peekable};

use crate::{
    chunk::Chunk,
    code::OpCode,
    environment::Environment,
    error::{Location, SiltError},
    lexer::{Lexer, TokenResult},
    token::Token,
    value::Value,
};

struct Compiler {
    chunk: Chunk,
    iterator: Option<Peekable<Lexer>>,
    // locations: Vec<Location>,
    previous: Location,
    // lexer: Lexer,
    // parser: Parser,
    // resolver: Resolver,
    // interpreter: Interpreter,
}
impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            chunk: Chunk::new(),
            iterator: None,
            // locations: vec![],
            previous: (0, 0),
            // lexer: Lexer::new(),
            // parser: Parser::new(),
            // resolver: Resolver::new(),
            // interpreter: Interpreter::new(),
        }
    }
    pub fn compile(&mut self, source: String, global: &mut Environment) -> Value {
        let mut lexer = Lexer::new(source.to_owned());

        self.iterator = Some(lexer.peekable());
        // lexer.enumerate().for_each(|(i, res)| match res {
        //     Ok((token, location)) => {
        //         self.iterator.push(t);
        //     }
        //     Err(e) => {
        //         println!("| {}", e);
        //     }
        // });

        // let mut parser = crate::parser::parser::Parser::new(t, p, global);
        // println!("|----------------");
        // let mut statements = parser.parse();
        // statements
        //     .iter()
        //     .enumerate()
        //     .for_each(|(i, e)| println!("| {} {}", i, e));
        // // println!("{}", exp);
        // // let val = parser.evaluate(exp);
        // let err = parser.get_errors();
        // if err.len() > 0 {
        //     println!("|----------------");
        //     println!("Parse Errors:");
        //     err.iter().for_each(|e| println!("{}", e));
        //     println!("-----------------");
        // } else {
        //     println!("-----------------");
        //     let mut resolver = crate::resolver::Resolver::new();
        //     resolver.process(&mut statements);
        //     let res = crate::interpreter::execute(global, &statements);
        //     match res {
        //         Ok(v) => {
        //             println!("");
        //             return v;
        //         }
        //         Err(e) => {
        //             println!("| Runtime Errors:");
        //             println!("| {}", e);
        //             println!("-----------------");
        //         }
        //     }
        // }

        self.expression();
        // t.iter().enumerate().for_each(|(i, t)| {
        //     let p = p.get(i).unwrap_or(&(0, 0));
        //     self.expression(t, p);
        //     // println!("|{}:{}| {}", p.0, p.1, t)
        // });

        self.emit(OpCode::RETURN, (0, 0));
        Value::Nil
    }

    fn expression(&mut self) {}

    fn emit(&mut self, op: OpCode, location: Location) {
        self.chunk.write_code(op, location);
    }
    fn constant(&mut self, value: Value, location: Location) {
        let constant = self.chunk.write_constant(value) as u8;
        self.emit(OpCode::CONSTANT { constant }, location);
    }

    fn current_chunk(&self) -> &Chunk {
        &self.chunk
    }

    fn eat(&mut self) {
        let p = self.iterator.unwrap().next();
        if let Some(Ok(t)) = p {
            self.previous = t.1;
        }
    }

    fn grouping(&mut self) {
        self.expression();
        expect_token!(self, CloseParen, SiltError::UnterminatedParenthesis(0, 0));
        // self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }
    // fn expression(&mut self) {}
}
