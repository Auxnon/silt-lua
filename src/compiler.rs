pub mod compiler {
    use std::{
        fmt::{Display, Formatter},
        iter::Peekable,
        mem::{swap, take},
        println, vec,
    };

    use crate::{
        chunk::Chunk,
        code::OpCode,
        environment::Environment,
        error::{ErrorTuple, Location, SiltError},
        lexer::{Lexer, TokenOption, TokenResult, TokenTuple},
        token::{Operator, Token},
        value::Value,
    };

    macro_rules! build_block_until {
        ($self:ident $($rule:ident)|*) => {{
            let mut statements = vec![];
            while !matches!($self.peek(), Some(
                $( &Token::$rule)|*)) {
                    statements.push($self.declaration());
            }
            statements
        }};
    }

    macro_rules! devout {
        ($self:ident $message:literal) => {
            #[cfg(feature = "dev-out")]
            println!("-> {}: {:?}", $message, $self.peek().unwrap_or(&Token::Nil));
        };
    }

    macro_rules! op_assign {
        ($self:ident, $ident:ident,$op:ident) => {{
            let value = $self.expression();
            let bin = Expression::Binary {
                left: Box::new(Expression::Variable {
                    $ident,
                    location: $self.get_last_loc(),
                }),
                operator: Operator::$op,
                right: Box::new(value),
                location: $self.get_last_loc(),
            };
            Expression::Assign {
                ident: $ident,
                value: Box::new(bin),
                location: $self.get_last_loc(),
            }
        }};
    }

    /** error if missing, eat if present */
    macro_rules! expect_token {
        ($self:ident $token:ident) => {{
            if let Some(&Token::$token) = $self.peek() {
                $self.eat();
            } else {
                $self.error(SiltError::ExpectedToken(Token::$token));
                return Statement::InvalidStatement;
            }
        };};
        ($self:ident, $token:ident, $custom_error:expr) => {{
            if let Some(&Token::$token) = $self.peek() {
                $self.eat();
            } else {
                return Err($custom_error);
            }
        };};
    }
    macro_rules! expect_token_exp {
        ($self:ident $token:ident) => {{
            if let Some(&Token::$token) = $self.peek() {
                $self.eat();
            } else {
                $self.error(SiltError::ExpectedToken(Token::$token));
                return Expression::InvalidExpression;
            }
        };};
    }

    macro_rules! rule {
        ($prefix:expr, $infix:expr, $precedence:tt) => {{
            ParseRule {
                prefix: $prefix,
                infix: $infix,
                precedence: Precedence::$precedence,
            }
        }};
        () => {};
    }

    #[derive(PartialEq, PartialOrd)]
    enum Precedence {
        None,
        Assignment, // =
        Or,         // or
        And,        // and
        Equality,   // == ~= !=
        Comparison, // < > <= >=
        Term,       // + -
        Factor,     // * /
        Unary,      // ~ - !
        Call,       // . ()
        Primary,
    }

    impl Precedence {
        fn next(self) -> Self {
            match self {
                Precedence::None => Precedence::Assignment,
                Precedence::Assignment => Precedence::Or,
                Precedence::Or => Precedence::And,
                Precedence::And => Precedence::Equality,
                Precedence::Equality => Precedence::Comparison,
                Precedence::Comparison => Precedence::Term,
                Precedence::Term => Precedence::Factor,
                Precedence::Factor => Precedence::Unary,
                Precedence::Unary => Precedence::Call,
                Precedence::Call => Precedence::Primary,
                Precedence::Primary => Precedence::Primary, // TODO over?
            }
        }
    }

    impl Display for Precedence {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                Precedence::None => write!(f, "None"),
                Precedence::Assignment => write!(f, "Assignment"),
                Precedence::Or => write!(f, "Or"),
                Precedence::And => write!(f, "And"),
                Precedence::Equality => write!(f, "Equality"),
                Precedence::Comparison => write!(f, "Comparison"),
                Precedence::Term => write!(f, "Term"),
                Precedence::Factor => write!(f, "Factor"),
                Precedence::Unary => write!(f, "Unary"),
                Precedence::Call => write!(f, "Call"),
                Precedence::Primary => write!(f, "Primary"),
            }
        }
    }

    struct ParseRule {
        prefix: fn(&mut Compiler),
        infix: fn(&mut Compiler),
        precedence: Precedence,
    }

    pub struct Compiler {
        chunk: Chunk,
        iterator: Peekable<Lexer>,
        pub current_index: usize,
        pub errors: Vec<ErrorTuple>,
        pub valid: bool,
        current: TokenTuple,
        previous: TokenTuple,
        pre_previous: TokenTuple,
        extra: bool, // pub global: &'a mut Environment,
    }
    impl<'a> Compiler {
        pub fn new() -> Compiler {
            // assert!(p.len() == p.len());
            Self {
                chunk: Chunk::new(),
                iterator: Lexer::new("".to_string()).peekable(),
                current: (Token::Nil, (0, 0)),
                current_index: 0,
                errors: vec![],
                valid: true,
                previous: (Token::Nil, (0, 0)),
                pre_previous: (Token::Nil, (0, 0)),
                extra: true,
            }
        }

        fn error_last(&mut self, code: SiltError) {
            self.valid = false;
            self.errors.push(ErrorTuple {
                code,
                location: self.pre_previous.1,
            });
        }
        fn error_syntax(&mut self, code: SiltError, location: Location) {
            self.valid = false;
            self.errors.push(ErrorTuple { code, location });
        }

        fn error(&mut self, code: SiltError) {
            self.valid = false;
            self.errors.push(ErrorTuple {
                code,
                location: self.previous.1,
            });
        }

        pub fn get_errors(&self) -> &Vec<ErrorTuple> {
            &self.errors
        }

        fn advance(&mut self) -> bool {
            self.current_index += 1;
            // self.current=self.iterator.next()
            match self.iterator.next() {
                Some(res) => match res {
                    Ok(t) => {
                        self.pre_previous = std::mem::replace(
                            &mut self.previous,
                            std::mem::replace(&mut self.current, t),
                        );
                    }
                    Err(e) => {
                        self.error_syntax(e.code, e.location);
                        return false;
                    }
                },
                None => {
                    self.pre_previous = std::mem::replace(
                        &mut self.previous,
                        std::mem::replace(&mut self.current, (Token::EOF, (0, 0))),
                    );
                    return false;
                }
            }
            println!(
                "advance: {}:{} {} -> {} -> {}",
                self.current.1 .0,
                self.current.1 .1,
                self.current.0,
                self.previous.0,
                self.pre_previous.0
            );
            // self.current
            // self.previous = take(&mut self.current);
            true
        }

        fn eat(&mut self) {
            self.current_index += 1;
            let p = self.iterator.next();
            if let Some(Ok(t)) = p {
                // assign to previous and old previous to pre_previous
                self.pre_previous = std::mem::replace(&mut self.previous, t);
            }
        }

        pub fn next(&mut self) -> TokenOption {
            self.current_index += 1;
            self.iterator.next()
        }

        /** only use after peek */
        pub fn eat_out(&mut self) -> TokenResult {
            self.current_index += 1;
            self.iterator.next().unwrap()
        }

        fn peek(&mut self) -> Option<&TokenResult> {
            self.iterator.peek()
        }

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

        fn is_end(&mut self) -> bool {
            match self.peek() {
                None => true,
                _ => false,
            }
        }

        fn get_loc(&self) -> Location {
            #[cfg(feature = "dev-out")]
            println!(
                "get index {} is loc {}:{}",
                self.current, self.locations[self.current].0, self.locations[self.current].1
            );
            // self.locations[self.current]
            // TODO current?
            self.previous.1
        }

        fn get_last_loc(&self) -> Location {
            #[cfg(feature = "dev-out")]
            println!(
                "get index {} is loc {}:{}",
                self.current - 1,
                self.locations[self.current - 1].0,
                self.locations[self.current - 1].1
            );
            self.previous.1
        }

        // pub fn parse(&mut self) -> Vec<Statement> {
        //     let mut statements = vec![];
        //     while !self.is_end() {
        //         // if let Ok(s) = self.statement() {
        //         statements.push(self.declaration());
        //         // }
        //         // else synchronize
        //     }
        //     statements
        // }

        fn get_rule(token: &Token) -> ParseRule {
            // ParseRule {
            //     prefix: Some(|self| self.grouping()),
            //     infix: None,
            //     precedence: Precedence::None,
            // },
            // let func: dyn FnMut(&mut Compiler<'_>) = &Self::grouping;
            // store reference of callable function within self
            // let func: &fn(&'a mut Compiler<'_>) = &Self::grouping as &fn(&'a mut Compiler<'_>);
            // let func: fn(&mut Compiler) = unary;

            // func(self);
            match token {
                Token::OpenParen => rule!(grouping, void, None),
                Token::Assign => rule!(void, void, None),
                Token::Op(op) => match op {
                    Operator::Sub => rule!(unary, binary, Term),
                    Operator::Add => rule!(void, binary, Term),
                    Operator::Multiply => rule!(void, binary, Factor),
                    Operator::Divide => rule!(void, binary, Factor),
                    Operator::Not => rule!(unary, void, None),
                    Operator::NotEqual => rule!(void, binary, Equality),
                    Operator::Equal => rule!(void, binary, Equality),
                    Operator::Less => rule!(void, binary, Comparison),
                    Operator::LessEqual => rule!(void, binary, Comparison),
                    Operator::Greater => rule!(void, binary, Comparison),
                    Operator::GreaterEqual => rule!(void, binary, Comparison),
                    _ => rule!(void, void, None),
                },
                Token::Integer(_) => rule!(integer, void, None),
                Token::Nil => rule!(literal, void, None),
                Token::True => rule!(literal, void, None),
                Token::False => rule!(literal, void, None),
                // Token::Bang => rule!(unary, void, None),
                _ => rule!(void, void, None),
            }
        }

        pub fn compile(&mut self, source: String, global: &mut Environment) -> Chunk {
            let lexer = Lexer::new(source.to_owned());
            lexer.for_each(|r| match r {
                Ok(t) => {
                    println!("token {}", t.0);
                }
                Err(e) => println!("err {}", e),
            });
            let lexer = Lexer::new(source.to_owned());

            self.iterator = lexer.peekable();

            // if Precedence::And > Precedence::Or {
            //     println!("and is greater than or");
            // }
            self.advance();

            while !self.is_end() {
                expression(self);
            }

            // while !self.is_end() {

            // }

            self.emit(OpCode::RETURN, (0, 0));

            std::mem::take(&mut self.chunk).into()
        }

        fn parse_precedence(&mut self, precedence: Precedence) {
            self.advance();
            let t = &self.previous.0;
            println!("check rule for token {}", t);
            let rule = Self::get_rule(t);
            // println!("precedence:  {}", precedence);
            // if (rule.prefix) != Self::void { // TODO bubble error up if no prefix, call invalid func to bubble?
            (rule.prefix)(self);

            while precedence <= Self::get_rule(&self.current.0).precedence {
                self.advance();
                let rule = Self::get_rule(&self.previous.0);
                (rule.infix)(self);
            }
        }
    }

    fn expression(this: &mut Compiler) {
        this.parse_precedence(Precedence::Assignment);
    }

    fn grouping(this: &mut Compiler) {
        expression(this);
        //TODO expect
        // expect_token!(self, CloseParen, SiltError::UnterminatedParenthesis(0, 0));
        // self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    /** op unary or primary */
    fn unary(this: &mut Compiler) {
        let t = &this.previous.clone();
        // self.expression();

        this.parse_precedence(Precedence::Unary);
        match t.0 {
            Token::Op(Operator::Sub) => this.emit(OpCode::NEGATE, t.1),
            Token::Op(Operator::Not) => this.emit(OpCode::NOT, t.1),
            _ => {}
        }
        //     let operator = Self::de_op(self.eat_out());
        //     let location = self.get_last_loc();
        //     let right = self.unary();
        //     Expression::Unary {
        //         operator,
        //         right: Box::new(right),
        //         location,
        //     }
        // } else {
        //     self.anonymous_check()
        // }
    }

    fn binary(this: &mut Compiler) {
        let t = this.previous.clone();
        let rule = Compiler::get_rule(&t.0);
        this.parse_precedence(rule.precedence.next());
        if let Token::Op(op) = t.0 {
            match op {
                Operator::Add => this.emit(OpCode::ADD, t.1),
                Operator::Sub => this.emit(OpCode::SUB, t.1),
                Operator::Multiply => this.emit(OpCode::MULTIPLY, t.1),
                Operator::Divide => this.emit(OpCode::DIVIDE, t.1),

                // Operator::Modulus => self.emit(OpCode::MODULUS, t.1),
                // Operator::Equal => self.emit(OpCode::EQUAL, t.1),
                Operator::Equal => this.emit(OpCode::EQUAL, t.1),
                Operator::NotEqual => this.emit(OpCode::NOT_EQUAL, t.1),
                Operator::Less => this.emit(OpCode::LESS, t.1),
                Operator::LessEqual => this.emit(OpCode::LESS_EQUAL, t.1),
                Operator::Greater => this.emit(OpCode::GREATER, t.1),
                Operator::GreaterEqual => this.emit(OpCode::GREATER_EQUAL, t.1),

                _ => todo!(),
            }
        }
    }

    fn integer(this: &mut Compiler) {
        let t = &this.previous;
        let value = if let Token::Integer(i) = t.0 {
            println!("integer: {}", i);
            Value::Integer(i)
        } else {
            unreachable!()
        };
        this.constant(value, t.1);
    }
    fn literal(this: &mut Compiler) {
        match this.previous.0 {
            Token::Nil => this.emit(OpCode::NIL, this.previous.1),
            Token::True => this.emit(OpCode::TRUE, this.previous.1),
            Token::False => this.emit(OpCode::FALSE, this.previous.1),
            _ => unreachable!(),
        }
    }

    pub fn void(_: &mut Compiler) {}

    pub fn invalid(_: &mut Compiler) { // TODO
                                       // this.error(SiltError::InvalidExpression);
    }
}

// declare
// if var  return declare_staement
// return statement
// declare_statement
// eat identifier
// if equal then expresion
// otherwise return as nil binary assign

// ======
// ======
// ======
// ======

// fn declaration(&mut self) -> Statement {
//     devout!(self "declaration");
//     match self.peek() {
//         Some(&Token::Local) => self.declaration_scope(true, false),
//         Some(&Token::Global) => self.declaration_scope(false, false),
//         Some(&Token::Function) => {
//             self.eat();
//             self.define_function(true, None)
//         }
//         _ => self.statement(),
//     }
// }

// fn declaration_scope(&mut self, local: bool, already_function: bool) -> Statement {
//     self.eat();
//     match self.eat_out() {
//         Token::Identifier(ident) => {
//             let ident = (self.global.to_register(&ident), 0);
//             self.typing(ident, local)
//         }
//         Token::Function => {
//             if !already_function {
//                 self.define_function(local, None)
//             } else {
//                 self.error(SiltError::ExpectedLocalIdentifier);
//                 Statement::InvalidStatement
//             }
//         }
//         _ => {
//             self.error(SiltError::ExpectedLocalIdentifier);
//             Statement::InvalidStatement
//         }
//     }
// }

// fn typing(&mut self, ident: Ident, local: bool) -> Statement {
//     if let Some(&Token::Colon) = self.peek() {
//         // typing or self calling
//         self.eat();
//         let token = self.eat_out();
//         if let Token::ColonIdentifier(target) = token {
//             // method or type name
//             // if let Some(&Token::OpenParen) = self.peek() {
//             //     // self call

//             //     Statement::InvalidStatement
//             // } else {
//             //     // typing
//             //     // return self.assign(self.peek(), ident);
//             //     Statement::InvalidStatement
//             // }
//             self.define_declaration(ident, local)
//         } else {
//             self.error(SiltError::InvalidColonPlacement);
//             Statement::InvalidStatement
//         }
//     } else {
//         self.define_declaration(ident, local)
//     }
// }

// fn assignment(&mut self) -> Expression {
//     devout!(self "assignment");
//     let exp = self.logical_or();
//     if let Some(
//         &Token::Assign
//         | &Token::AddAssign
//         | &Token::SubAssign
//         | &Token::MultiplyAssign
//         | &Token::DivideAssign
//         | &Token::ModulusAssign,
//     ) = self.peek()
//     {
//         //let ident= current somehow?? use the exp as ident?
//         return if let Expression::Variable { ident, location } = exp {
//             self.assigner(ident)
//         } else {
//             let t = self.peek().unwrap().clone();
//             self.error(SiltError::InvalidAssignment(t));
//             Expression::Literal {
//                 value: Value::Nil,
//                 location: self.get_last_loc(),
//             }
//         };
//     }
//     exp
// }

// fn define_declaration(&mut self, ident: Ident, local: bool) -> Statement {
//     let t = self.peek();
//     match t {
//         Some(&Token::Assign) => Statement::Declare {
//             ident,
//             local,
//             expr: Box::new(self.next_expression()),
//         },
//         // we can't increment what doesn't exist yet, like what are you even doing?
//         Some(
//             &Token::AddAssign
//             | &Token::SubAssign
//             | &Token::MultiplyAssign
//             | &Token::DivideAssign
//             | &Token::ModulusAssign,
//         ) => {
//             let tt = t.unwrap().clone();
//             self.error(SiltError::InvalidAssignment(tt));
//             Statement::InvalidStatement
//         }
//         _ => Statement::Declare {
//             ident,
//             local,
//             expr: Box::new(Expression::Literal {
//                 value: Value::Nil,
//                 location: self.get_last_loc(),
//             }),
//         },
//     }
// }

// fn define_function(&mut self, local: bool, pre_ident: Option<usize>) -> Statement {
//     // self.eat(); // parser callers have already eaten token, they're full! lol
//     let location = self.get_last_loc();
//     if let Token::Identifier(ident) = self.eat_out() {
//         let ident = (self.global.to_register(&ident), 0);
//         let func = self.function_expression(location);
//         return Statement::Declare {
//             ident,
//             local,
//             expr: Box::new(func),
//         };
//     }
//     self.error(SiltError::ExpectedLocalIdentifier);
//     Statement::InvalidStatement
// }

// fn function_expression(&mut self, location: Location) -> Expression {
//     let mut params = vec![];

//     expect_token_exp!(self OpenParen);

//     if let Some(&Token::CloseParen) = self.peek() {
//         self.eat();
//     } else {
//         while let Token::Identifier(ident) = self.eat_out() {
//             let ident = self.global.to_register(&ident);
//             params.push(ident);
//             if let Some(&Token::Comma) = self.peek() {
//                 self.eat();
//             } else {
//                 break;
//             }
//         }
//         // TODO specific terminating paren error
//         expect_token_exp!(self CloseParen);
//     }
//     let block = self.block();
//     let func = Rc::new(Function::new(params, block));
//     Expression::Function {
//         value: func,
//         location,
//     }
// }

// fn assigner(&mut self, ident: Ident) -> Expression {
//     let tok = self.eat_out();

//     let location = self.get_last_loc();
//     match tok {
//         Token::Assign => Expression::Assign {
//             ident,
//             value: Box::new(self.expression()),
//             location,
//         },
//         Token::AddAssign => {
//             op_assign!(self, ident, Add)
//         }
//         Token::SubAssign => {
//             op_assign!(self, ident, Sub)
//         }
//         Token::MultiplyAssign => {
//             op_assign!(self, ident, Multiply)
//         }
//         Token::DivideAssign => {
//             op_assign!(self, ident, Divide)
//         }
//         Token::ModulusAssign => {
//             op_assign!(self, ident, Modulus)
//         }
//         _ => panic!("impossible"), //Statement::Expression(Expression::Variable {ident})
//     }
// }

// //////////////////////////////
// /// Statements
// //////////////////////////////

// fn statement(&mut self) -> Statement {
//     devout!(self "statement");
//     match self.peek() {
//         Some(&Token::If) => self.if_statement(),
//         Some(&Token::While) => self.while_statement(),
//         Some(&Token::For) => self.for_statement(),
//         Some(&Token::Print) => Statement::Print(Box::new(self.next_expression())),
//         Some(&Token::Do) => Statement::Block(self.eat_block()),
//         Some(&Token::Return) => self.return_statement(),
//         // Some(&Token::Function) => self.define_function(false, None),
//         Some(&Token::SemiColon) => {
//             self.eat();
//             // worked our way into a corner with this one huh?
//             Statement::Skip
//         }

//         _ => Statement::Expression(Box::new(self.expression())), // don't eat
//     }
// }

// /** eat token, collect statements until hitting end, error if no end hit */
// fn eat_block(&mut self) -> Vec<Statement> {
//     self.eat();
//     self.block()
// }

// /** collect statements until hitting end, error if no end hit */
// fn block(&mut self) -> Vec<Statement> {
//     let statements = build_block_until!(self End);

//     if !matches!(self.eat_out(), Token::End) {
//         self.error(SiltError::UnterminatedBlock);
//     }
//     statements
// }

// fn if_statement(&mut self) -> Statement {
//     self.eat();
//     let condition = self.expression();
//     if let Some(&Token::Then) = self.peek() {
//         self.eat();

//         let then_branch = build_block_until!(self End | Else | ElseIf);
//         match self.peek() {
//             Some(&Token::Else) => {
//                 self.eat();
//                 let else_branch = build_block_until!(self End);

//                 self.eat();
//                 Statement::If {
//                     cond: Box::new(condition),
//                     then: then_branch,
//                     else_cond: Some(else_branch),
//                 }
//             }
//             Some(&Token::ElseIf) => {
//                 // self.eat();
//                 let nested = vec![self.if_statement()];
//                 Statement::If {
//                     cond: Box::new(condition),
//                     then: then_branch,
//                     else_cond: Some(nested),
//                 }
//             }
//             Some(&Token::End) => {
//                 self.eat();
//                 Statement::If {
//                     cond: Box::new(condition),
//                     then: then_branch,
//                     else_cond: None,
//                 }
//             }
//             _ => {
//                 self.error(SiltError::UnterminatedBlock);
//                 Statement::InvalidStatement
//             }
//         }
//     } else {
//         self.error(SiltError::ExpectedThen);
//         Statement::InvalidStatement
//     }
// }

// fn while_statement(&mut self) -> Statement {
//     self.eat();
//     let cond = self.expression();
//     if let Some(&Token::Do) = self.peek() {
//         let block = self.eat_block();
//         Statement::While {
//             cond: Box::new(cond),
//             block,
//         }
//     } else {
//         self.error(SiltError::ExpectedDo);
//         Statement::InvalidStatement
//     }
// }

// fn for_statement(&mut self) -> Statement {
//     // Statement::InvalidStatement
//     self.eat();
//     let ident = self.eat_out();
//     if let Token::Identifier(ident_str) = ident {
//         let ident = self.global.to_register(&ident_str);
//         expect_token!(self Assign);
//         let start = Box::new(self.expression());
//         expect_token!(self Comma);
//         let end = Box::new(self.expression());
//         let step = if let Some(&Token::Comma) = self.peek() {
//             self.eat();
//             Some(Box::new(self.expression()))
//         } else {
//             None
//         };
//         return if let Some(&Token::Do) = self.peek() {
//             let block = self.eat_block();
//             Statement::NumericFor {
//                 ident,
//                 start,
//                 end,
//                 step,
//                 block,
//             }
//         } else {
//             self.error(SiltError::ExpectedDo);
//             Statement::InvalidStatement
//         };
//     } else {
//         self.error(SiltError::ExpectedLocalIdentifier);
//     }
//     Statement::InvalidStatement
// }

// fn return_statement(&mut self) -> Statement {
//     self.eat();
//     let value = if let Some(&Token::SemiColon | &Token::End) = self.peek() {
//         Expression::Literal {
//             value: Value::Nil,
//             location: self.get_last_loc(),
//         }
//     } else {
//         self.expression()
//     };
//     Statement::Return(Box::new(value))

//     // Statement::Return(Box::new(self.next_expression()))
// }

// fn next_expression(&mut self) -> Expression {
//     self.eat();
//     self.expression()
// }

// fn expression(&mut self) -> Expression {
//     devout!(self "expression");
//     self.assignment()
// }

// fn logical_or(&mut self) -> Expression {
//     let mut exp = self.logical_and();
//     while let Some(&Token::Op(Operator::Or)) = self.peek() {
//         self.eat();
//         let right = self.logical_and();
//         exp = Expression::Logical {
//             left: Box::new(exp),
//             operator: Operator::Or,
//             right: Box::new(right),
//             location: self.get_last_loc(),
//         };
//     }
//     exp
// }

// fn logical_and(&mut self) -> Expression {
//     let mut exp = self.equality();
//     while let Some(&Token::Op(Operator::And)) = self.peek() {
//         self.eat();
//         let right = self.equality();
//         exp = Expression::Logical {
//             left: Box::new(exp),
//             operator: Operator::And,
//             right: Box::new(right),
//             location: self.get_last_loc(),
//         };
//     }
//     exp
// }

// fn equality(&mut self) -> Expression {
//     let mut exp = self.comparison();
//     while let Some(&Token::Op(Operator::NotEqual | Operator::Equal)) = self.peek() {
//         let operator = Self::de_op(self.eat_out());
//         let location = self.get_last_loc();
//         let right = self.comparison();
//         exp = Expression::Binary {
//             left: Box::new(exp),
//             operator,
//             right: Box::new(right),
//             location,
//         };
//     }
//     exp
// }

// fn comparison(&mut self) -> Expression {
//     let mut exp = self.term();

//     while let Some(&Token::Op(
//         Operator::Less | Operator::LessEqual | Operator::Greater | Operator::GreaterEqual,
//     )) = self.peek()
//     {
//         let operator = Self::de_op(self.eat_out());
//         let location = self.get_last_loc();
//         let right = self.term();
//         exp = Expression::Binary {
//             left: Box::new(exp),
//             operator,
//             right: Box::new(right),
//             location,
//         };
//     }
//     exp
// }

// fn term(&mut self) -> Expression {
//     let mut exp = self.factor();
//     while let Some(&Token::Op(Operator::Add | Operator::Sub | Operator::Concat)) =
//         self.peek()
//     {
//         let operator = Self::de_op(self.eat_out());
//         let location = self.get_last_loc();
//         let right = self.factor();
//         exp = Expression::Binary {
//             left: Box::new(exp),
//             operator,
//             right: Box::new(right),
//             location,
//         };
//     }
//     exp
// }

// fn factor(&mut self) -> Expression {
//     let mut exp = self.unary();
//     while let Some(&Token::Op(Operator::Multiply | Operator::Divide | Operator::Modulus)) =
//         self.peek()
//     {
//         let operator = Self::de_op(self.eat_out());
//         let right = self.unary();
//         let location = self.get_last_loc();
//         exp = Expression::Binary {
//             left: Box::new(exp),
//             operator,
//             right: Box::new(right),
//             location,
//         };
//     }
//     exp
// }

// fn anonymous_check(&mut self) -> Expression {
//     let exp = self.primary();
//     if let Some(&Token::ArrowFunction) = self.peek() {
//         let location = self.get_loc();
//         self.eat();
//         let params = if let Expression::Variable { ident, location } = exp {
//             vec![ident.0]
//         } else {
//             vec![]
//         };
//         let block = self.block();
//         let func = Rc::new(Function::new(params, block));
//         return Expression::Function {
//             value: func,
//             location,
//         };
//     } else {
//         self.call(exp)
//     }
// }

// fn call(&mut self, mut exp: Expression) -> Expression {
//     while match self.peek() {
//         Some(&Token::OpenParen) => {
//             devout!(self "call");
//             //TODO while(true) with break but also calls the finishCall func?
//             let start = self.get_loc();
//             match self.arguments(start) {
//                 Ok(args) => {
//                     exp = Expression::Call {
//                         callee: Box::new(exp),
//                         args,
//                         location: start,
//                     }
//                 }
//                 Err(e) => {
//                     self.error(e);
//                     return Expression::InvalidExpression;
//                 }
//             }
//             true
//         }
//         Some(&Token::StringLiteral(_)) => {
//             devout!(self "call");
//             let start = self.get_loc();
//             if let Some(Token::StringLiteral(s)) = self.eat() {
//                 let args = vec![Expression::Literal {
//                     value: Value::String(s),
//                     location: start,
//                 }];
//                 exp = Expression::Call {
//                     callee: Box::new(exp),
//                     args,
//                     location: start,
//                 }
//             }
//             true
//         }
//         _ => false,
//     } {}
//     exp
// }

// fn arguments(&mut self, start: Location) -> Result<Vec<Expression>, SiltError> {
//     self.eat();
//     let mut args = vec![];
//     while !matches!(self.peek(), Some(&Token::CloseParen)) {
//         args.push(self.expression());
//         if let Some(&Token::Comma) = self.peek() {
//             self.eat();
//         }
//     }
//     devout!(self "arguments");

//     expect_token!(
//         self,
//         CloseParen,
//         SiltError::UnterminatedParenthesis(start.0, start.1)
//     );

//     Ok(args)
// }
// fn primary(&mut self) -> Expression {
//     // Err(code) => {
//     //     println!("Error Heere: {} :{}", code, self.current);
//     //     self.error(code);
//     //     Expression::InvalidExpression
//     // }
//     devout!(self "primary");

//     let t = self.next();
//     let location = self.get_last_loc();
//     // errors will 1 ahead, use error_last
//     match t {
//         Some(Token::Number(n)) => Expression::Literal {
//             value: Value::Number(n),
//             location,
//         },
//         Some(Token::StringLiteral(s)) => Expression::Literal {
//             value: Value::String(s),
//             location,
//         },
//         Some(Token::Integer(i)) => Expression::Literal {
//             value: Value::Integer(i),
//             location,
//         },
//         Some(Token::True) => Expression::Literal {
//             value: Value::Bool(true),
//             location,
//         },
//         Some(Token::False) => Expression::Literal {
//             value: Value::Bool(false),
//             location,
//         },
//         Some(Token::Nil) => Expression::Literal {
//             value: Value::Nil,
//             location,
//         },

//         Some(Token::OpenParen) => {
//             let start = self.get_last_loc(); // we're ahead normally, in this func we're ahead by 2 as we already ate, yummers
//             let exp = self.expression();
//             if let Some(Token::CloseParen) = self.peek() {
//                 self.eat();
//                 Expression::GroupingExpression {
//                     expression: Box::new(exp),
//                     location: start,
//                 }
//             } else {
//                 self.error(SiltError::UnterminatedParenthesis(start.0, start.1));
//                 Expression::InvalidExpression
//             }
//         }
//         Some(Token::Identifier(ident)) => Expression::Variable {
//             ident: (self.global.to_register(&ident), 0),
//             location,
//         },
//         Some(Token::Function) => self.function_expression(location),
//         // Some(Token::EOF) => Ok(Expression::EndOfFile),
//         Some(Token::Op(o)) => {
//             self.error(SiltError::ExpInvalidOperator(o));
//             Expression::InvalidExpression
//         }
//         Some(tt) => {
//             // TODO nil?
//             self.error(SiltError::InvalidTokenPlacement(tt));
//             Expression::InvalidExpression
//         }
//         None => {
//             self.error_last(SiltError::EarlyEndOfFile);
//             Expression::InvalidExpression
//         }
//     }
// }

// fn de_op(t: Token) -> Operator {
//     if let Token::Op(o) = t {
//         return o;
//     }
//     panic!("Bad operator") // can this happen
// }
