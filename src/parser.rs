pub mod parser {
    use std::{println, rc::Rc, vec};

    use crate::{
        environment::Environment,
        error::{ErrorTuple, Location, SiltError},
        expression::{Expression, Ident},
        function::Function,
        statement::Statement,
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
    // macro_rules! val_err {
    //     ($left:ident,$right:ident) => {
    //         return Err(SiltError::ExpAddValueWithValue(
    //             Value::String($left),
    //             Value::String($right),
    //         ));
    //     };
    // }
    // macro_rules! op_assign {
    //     ($self:ident, $ident:ident,$op:ident) => {{
    //         let value = $self.next_expression();
    //         Statement::Declare {
    //             ident: $ident.clone(),
    //             value: Expression::Binary {
    //                 left: Box::new(Expression::Variable { $ident }),
    //                 operator: Operator::$op,
    //                 right: Box::new(value),
    //             },
    //         }
    //     }};
    // }

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

    pub struct Parser<'a> {
        pub iterator: std::iter::Peekable<std::vec::IntoIter<Token>>,
        pub locations: Vec<Location>,
        pub current: usize,
        pub errors: Vec<ErrorTuple>,
        pub valid: bool,
        pub global: &'a mut Environment,
    }
    impl<'a> Parser<'a> {
        pub fn new(t: Vec<Token>, p: Vec<Location>, global: &'a mut Environment) -> Parser<'a> {
            assert!(p.len() == p.len());
            let ee = t.into_iter().peekable();
            // let tt = t.iter().peekable();
            Parser {
                iterator: ee,
                locations: p,
                current: 0,
                errors: vec![],
                valid: true,
                global,
            }
        }

        // pub fn advance(&mut self) -> Option<Token> {
        //     self.current += 1;
        //     self.iterator.next().cloned()
        // }

        fn error_last(&mut self, code: SiltError) {
            self.valid = false;
            self.errors.push(ErrorTuple {
                code,
                location: self.locations[self.current - 2],
            });
        }

        fn error(&mut self, code: SiltError) {
            self.valid = false;
            self.errors.push(ErrorTuple {
                code,
                location: self.locations[self.current - 1],
            });
        }

        pub fn get_errors(&self) -> &Vec<ErrorTuple> {
            &self.errors
        }

        fn synchronize(&mut self) {
            while match self.peek() {
                Some(&Token::Class)
                | Some(&Token::Function)
                | Some(&Token::Do)
                | Some(&Token::For)
                | Some(&Token::If)
                | Some(&Token::While)
                | Some(&Token::Print)
                | Some(&Token::Return) => false,
                _ => true,
            } {
                self.eat();
            }
        }

        // _ => Value::Nil,

        // fn next_statement(&self) -> bool {}

        pub fn eat(&mut self) -> Option<Token> {
            self.current += 1;
            self.iterator.next()
            // println!(
            //     "{}",
            //     match self.iterator.next() {
            //         Some(t) => format!("{}", t),
            //         None => format!("None"),
            //     }
            // );
        }

        pub fn next(&mut self) -> Option<Token> {
            self.current += 1;
            self.iterator.next()
        }

        /** only use after peek */
        pub fn eat_out(&mut self) -> Token {
            self.current += 1;
            self.iterator.next().unwrap()
        }

        fn peek(&mut self) -> Option<&Token> {
            self.iterator.peek()
        }

        fn is_end(&mut self) -> bool {
            match self.peek() {
                None => true,
                _ => false,
            }
        }

        // fn get_last_loc(&self) -> Location {
        //     self.locations[self.current - 1]
        // }
        fn get_loc(&self) -> Location {
            #[cfg(feature = "dev-out")]
            println!(
                "get index {} is loc {}:{}",
                self.current, self.locations[self.current].0, self.locations[self.current].1
            );
            self.locations[self.current]
        }

        fn get_last_loc(&self) -> Location {
            #[cfg(feature = "dev-out")]
            println!(
                "get index {} is loc {}:{}",
                self.current - 1,
                self.locations[self.current - 1].0,
                self.locations[self.current - 1].1
            );
            self.locations[self.current - 1]
        }

        pub fn parse(&mut self) -> Vec<Statement> {
            let mut statements = vec![];
            while !self.is_end() {
                // if let Ok(s) = self.statement() {
                statements.push(self.declaration());
                // }
                // else synchronize
            }
            statements
        }
        // declare
        // if var  return declare_staement
        // return statement
        // declare_statement
        // eat identifier
        // if equal then expresion
        // otherwise return as nil binary assign

        fn declaration(&mut self) -> Statement {
            devout!(self "declaration");
            // if let Some(&Token::Local) = self.peek() {
            //     self.eat();
            match self.peek() {
                Some(&Token::Local) => self.declaration_scope(true, false),
                Some(&Token::Global) => self.declaration_scope(false, false),
                Some(&Token::Function) => {
                    self.eat();
                    self.define_function(true, None)
                }
                _ => self.statement(),
            }

            // if let Some(&Token::Local | &Token::Global) = self.peek() {
            //     let scope = self.eat_out();
            //     if let Some(Token::Identifier(ident)) = self.eat() {
            //         let ident = self.global.to_register(&ident);

            //     } else {
            //         self.error(SiltError::ExpectedLocalIdentifier);
            //         Statement::InvalidStatement
            //     }
            // } else {
            //     self.statement()
            // }
        }

        fn declaration_scope(&mut self, local: bool, already_function: bool) -> Statement {
            self.eat();
            match self.eat_out() {
                Token::Identifier(ident) => {
                    let ident = (self.global.to_register(&ident), 0);
                    self.typing(ident, local)
                }
                Token::Function => {
                    if !already_function {
                        self.define_function(local, None)
                    } else {
                        self.error(SiltError::ExpectedLocalIdentifier);
                        Statement::InvalidStatement
                    }
                }
                _ => {
                    self.error(SiltError::ExpectedLocalIdentifier);
                    Statement::InvalidStatement
                }
            }
        }

        fn typing(&mut self, ident: Ident, local: bool) -> Statement {
            if let Some(&Token::Colon) = self.peek() {
                // typing or self calling
                self.eat();
                let token = self.eat_out();
                if let Token::ColonIdentifier(target) = token {
                    // method or type name
                    // if let Some(&Token::OpenParen) = self.peek() {
                    //     // self call

                    //     Statement::InvalidStatement
                    // } else {
                    //     // typing
                    //     // return self.assign(self.peek(), ident);
                    //     Statement::InvalidStatement
                    // }
                    self.define_declaration(ident, local)
                } else {
                    self.error(SiltError::InvalidColonPlacement);
                    Statement::InvalidStatement
                }
            } else {
                self.define_declaration(ident, local)
            }
        }

        fn assignment(&mut self) -> Expression {
            devout!(self "assignment");
            let exp = self.logical_or();
            if let Some(
                &Token::Assign
                | &Token::AddAssign
                | &Token::SubAssign
                | &Token::MultiplyAssign
                | &Token::DivideAssign
                | &Token::ModulusAssign,
            ) = self.peek()
            {
                //let ident= current somehow?? use the exp as ident?
                return if let Expression::Variable { ident, location } = exp {
                    self.assigner(ident)
                } else {
                    let t = self.peek().unwrap().clone();
                    self.error(SiltError::InvalidAssignment(t));
                    Expression::Literal {
                        value: Value::Nil,
                        location: self.get_last_loc(),
                    }
                };
            }
            exp
        }

        fn define_declaration(&mut self, ident: Ident, local: bool) -> Statement {
            let t = self.peek();
            match t {
                Some(&Token::Assign) => Statement::Declare {
                    ident,
                    local,
                    expr: Box::new(self.next_expression()),
                },
                // we can't increment what doesn't exist yet, like what are you even doing?
                Some(
                    &Token::AddAssign
                    | &Token::SubAssign
                    | &Token::MultiplyAssign
                    | &Token::DivideAssign
                    | &Token::ModulusAssign,
                ) => {
                    let tt = t.unwrap().clone();
                    self.error(SiltError::InvalidAssignment(tt));
                    Statement::InvalidStatement
                }
                _ => Statement::Declare {
                    ident,
                    local,
                    expr: Box::new(Expression::Literal {
                        value: Value::Nil,
                        location: self.get_last_loc(),
                    }),
                },
            }
        }

        fn define_function(&mut self, local: bool, pre_ident: Option<usize>) -> Statement {
            // self.eat(); // parser callers have already eaten token, they're full! lol
            let location = self.get_last_loc();
            if let Token::Identifier(ident) = self.eat_out() {
                let ident = (self.global.to_register(&ident), 0);
                let func = self.function_expression(location);
                return Statement::Declare {
                    ident,
                    local,
                    expr: Box::new(func),
                };
            }
            self.error(SiltError::ExpectedLocalIdentifier);
            Statement::InvalidStatement
        }

        fn function_expression(&mut self, location: Location) -> Expression {
            let mut params = vec![];

            expect_token_exp!(self OpenParen);

            if let Some(&Token::CloseParen) = self.peek() {
                self.eat();
            } else {
                while let Token::Identifier(ident) = self.eat_out() {
                    let ident = self.global.to_register(&ident);
                    params.push(ident);
                    if let Some(&Token::Comma) = self.peek() {
                        self.eat();
                    } else {
                        break;
                    }
                }
                // TODO specific terminating paren error
                expect_token_exp!(self CloseParen);
            }
            let block = self.block();
            let func = Rc::new(Function::new(params, block));
            Expression::Function {
                value: func,
                location,
            }
        }

        fn assigner(&mut self, ident: Ident) -> Expression {
            let tok = self.eat_out();

            let location = self.get_last_loc();
            match tok {
                Token::Assign => Expression::Assign {
                    ident,
                    value: Box::new(self.expression()),
                    location,
                },
                Token::AddAssign => {
                    op_assign!(self, ident, Add)
                }
                Token::SubAssign => {
                    op_assign!(self, ident, Sub)
                }
                Token::MultiplyAssign => {
                    op_assign!(self, ident, Multiply)
                }
                Token::DivideAssign => {
                    op_assign!(self, ident, Divide)
                }
                Token::ModulusAssign => {
                    op_assign!(self, ident, Modulus)
                }
                _ => panic!("impossible"), //Statement::Expression(Expression::Variable {ident})
            }
        }

        // fn assign(&mut self, ident: usize) -> Statement {
        //     println!("-> assign {}", ident);
        //     match self.peek() {
        //         Some(&Token::Assign) => Statement::Declare {
        //             ident,
        //             value: Box::new(self.next_expression()),
        //         },
        //         Some(&Token::AddAssign) => {
        //             op_assign!(self, ident, Add)
        //         }
        //         Some(&Token::SubAssign) => {
        //             op_assign!(self, ident, Sub)
        //         }
        //         Some(&Token::MultiplyAssign) => {
        //             op_assign!(self, ident, Multiply)
        //         }
        //         Some(&Token::DivideAssign) => {
        //             op_assign!(self, ident, Divide)
        //         }
        //         Some(&Token::ModulusAssign) => {
        //             op_assign!(self, ident, Modulus)
        //         }
        //         _ => self.statement(), //Statement::Expression(Expression::Variable {ident})
        //     }
        // }

        //////////////////////////////
        /// Statements
        //////////////////////////////

        fn statement(&mut self) -> Statement {
            devout!(self "statement");
            match self.peek() {
                Some(&Token::If) => self.if_statement(),
                Some(&Token::While) => self.while_statement(),
                Some(&Token::For) => self.for_statement(),
                Some(&Token::Print) => Statement::Print(Box::new(self.next_expression())),
                Some(&Token::Do) => Statement::Block(self.eat_block()),
                Some(&Token::Return) => self.return_statement(),
                // Some(&Token::Function) => self.define_function(false, None),
                Some(&Token::SemiColon) => {
                    self.eat();
                    // worked our way into a corner with this one huh?
                    Statement::Skip
                }

                _ => Statement::Expression(Box::new(self.expression())), // don't eat
            }
        }

        // fn next_block(&mut self, end:W ) -> Vec<Statement>
        //     where W : Option<<&Token as BitOr<&Token>>::Output> {
        //     let d = Some(&Token::End | &Token::Else);
        //     let mut statements = vec![];
        //     while !matches!(self.peek(), Some(end)) {
        //         statements.push(self.local_declaration());
        //     }
        //
        //     if !matches!(self.eat_out(), Token::End) {
        //         self.error(SiltError::UnterminatedBlock);
        //     }
        //     statements
        // }

        /** eat token, collect statements until hitting end, error if no end hit */
        fn eat_block(&mut self) -> Vec<Statement> {
            self.eat();
            self.block()
        }

        /** collect statements until hitting end, error if no end hit */
        fn block(&mut self) -> Vec<Statement> {
            let statements = build_block_until!(self End);

            if !matches!(self.eat_out(), Token::End) {
                self.error(SiltError::UnterminatedBlock);
            }
            statements
        }

        fn if_statement(&mut self) -> Statement {
            self.eat();
            let condition = self.expression();
            if let Some(&Token::Then) = self.peek() {
                self.eat();

                let then_branch = build_block_until!(self End | Else | ElseIf);
                match self.peek() {
                    Some(&Token::Else) => {
                        self.eat();
                        let else_branch = build_block_until!(self End);

                        self.eat();
                        Statement::If {
                            cond: Box::new(condition),
                            then: then_branch,
                            else_cond: Some(else_branch),
                        }
                    }
                    Some(&Token::ElseIf) => {
                        // self.eat();
                        let nested = vec![self.if_statement()];
                        Statement::If {
                            cond: Box::new(condition),
                            then: then_branch,
                            else_cond: Some(nested),
                        }
                    }
                    Some(&Token::End) => {
                        self.eat();
                        Statement::If {
                            cond: Box::new(condition),
                            then: then_branch,
                            else_cond: None,
                        }
                    }
                    _ => {
                        self.error(SiltError::UnterminatedBlock);
                        Statement::InvalidStatement
                    }
                }
            } else {
                self.error(SiltError::ExpectedThen);
                Statement::InvalidStatement
            }
        }

        fn while_statement(&mut self) -> Statement {
            self.eat();
            let cond = self.expression();
            if let Some(&Token::Do) = self.peek() {
                let block = self.eat_block();
                Statement::While {
                    cond: Box::new(cond),
                    block,
                }
            } else {
                self.error(SiltError::ExpectedDo);
                Statement::InvalidStatement
            }
        }

        fn for_statement(&mut self) -> Statement {
            // Statement::InvalidStatement
            self.eat();
            let ident = self.eat_out();
            if let Token::Identifier(ident_str) = ident {
                let ident = self.global.to_register(&ident_str);
                expect_token!(self Assign);
                let start = Box::new(self.expression());
                expect_token!(self Comma);
                let end = Box::new(self.expression());
                let step = if let Some(&Token::Comma) = self.peek() {
                    self.eat();
                    Some(Box::new(self.expression()))
                } else {
                    None
                };
                return if let Some(&Token::Do) = self.peek() {
                    let block = self.eat_block();
                    Statement::NumericFor {
                        ident,
                        start,
                        end,
                        step,
                        block,
                    }
                } else {
                    self.error(SiltError::ExpectedDo);
                    Statement::InvalidStatement
                };
            } else {
                self.error(SiltError::ExpectedLocalIdentifier);
            }
            Statement::InvalidStatement
        }

        fn return_statement(&mut self) -> Statement {
            self.eat();
            let value = if let Some(&Token::SemiColon | &Token::End) = self.peek() {
                Expression::Literal {
                    value: Value::Nil,
                    location: self.get_last_loc(),
                }
            } else {
                self.expression()
            };
            Statement::Return(Box::new(value))

            // Statement::Return(Box::new(self.next_expression()))
        }

        fn next_expression(&mut self) -> Expression {
            self.eat();
            self.expression()
        }

        fn expression(&mut self) -> Expression {
            devout!(self "expression");
            self.assignment()
        }

        fn logical_or(&mut self) -> Expression {
            let mut exp = self.logical_and();
            while let Some(&Token::Op(Operator::Or)) = self.peek() {
                self.eat();
                let right = self.logical_and();
                exp = Expression::Logical {
                    left: Box::new(exp),
                    operator: Operator::Or,
                    right: Box::new(right),
                    location: self.get_last_loc(),
                };
            }
            exp
        }

        fn logical_and(&mut self) -> Expression {
            let mut exp = self.equality();
            while let Some(&Token::Op(Operator::And)) = self.peek() {
                self.eat();
                let right = self.equality();
                exp = Expression::Logical {
                    left: Box::new(exp),
                    operator: Operator::And,
                    right: Box::new(right),
                    location: self.get_last_loc(),
                };
            }
            exp
        }

        fn equality(&mut self) -> Expression {
            let mut exp = self.comparison();
            while let Some(&Token::Op(Operator::NotEqual | Operator::Equal)) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let location = self.get_last_loc();
                let right = self.comparison();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                    location,
                };
            }
            exp
        }

        fn comparison(&mut self) -> Expression {
            let mut exp = self.term();

            while let Some(&Token::Op(
                Operator::Less | Operator::LessEqual | Operator::Greater | Operator::GreaterEqual,
            )) = self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let location = self.get_last_loc();
                let right = self.term();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                    location,
                };
            }
            exp
        }

        fn term(&mut self) -> Expression {
            let mut exp = self.factor();
            while let Some(&Token::Op(Operator::Add | Operator::Sub | Operator::Concat)) =
                self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let location = self.get_last_loc();
                let right = self.factor();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                    location,
                };
            }
            exp
        }

        fn factor(&mut self) -> Expression {
            let mut exp = self.unary();
            while let Some(&Token::Op(Operator::Multiply | Operator::Divide | Operator::Modulus)) =
                self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let right = self.unary();
                let location = self.get_last_loc();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                    location,
                };
            }
            exp
        }

        // op unary or primary
        pub fn unary(&mut self) -> Expression {
            if let Some(&Token::Op(Operator::Sub | Operator::Not | Operator::Tilde)) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let location = self.get_last_loc();
                let right = self.unary();
                Expression::Unary {
                    operator,
                    right: Box::new(right),
                    location,
                }
            } else {
                self.anonymous_check()
            }
        }

        fn anonymous_check(&mut self) -> Expression {
            let exp = self.primary();
            if let Some(&Token::ArrowFunction) = self.peek() {
                let location = self.get_loc();
                self.eat();
                let params = if let Expression::Variable { ident, location } = exp {
                    vec![ident.0]
                } else {
                    vec![]
                };
                let block = self.block();
                let func = Rc::new(Function::new(params, block));
                return Expression::Function {
                    value: func,
                    location,
                };
            } else {
                self.call(exp)
            }
        }

        fn call(&mut self, mut exp: Expression) -> Expression {
            while match self.peek() {
                Some(&Token::OpenParen) => {
                    devout!(self "call");
                    //TODO while(true) with break but also calls the finishCall func?
                    let start = self.get_loc();
                    match self.arguments(start) {
                        Ok(args) => {
                            exp = Expression::Call {
                                callee: Box::new(exp),
                                args,
                                location: start,
                            }
                        }
                        Err(e) => {
                            self.error(e);
                            return Expression::InvalidExpression;
                        }
                    }
                    true
                }
                Some(&Token::StringLiteral(_)) => {
                    devout!(self "call");
                    let start = self.get_loc();
                    if let Some(Token::StringLiteral(s)) = self.eat() {
                        let args = vec![Expression::Literal {
                            value: Value::String(s),
                            location: start,
                        }];
                        exp = Expression::Call {
                            callee: Box::new(exp),
                            args,
                            location: start,
                        }
                    }
                    true
                }
                _ => false,
            } {}
            exp
        }

        fn arguments(&mut self, start: Location) -> Result<Vec<Expression>, SiltError> {
            self.eat();
            let mut args = vec![];
            while !matches!(self.peek(), Some(&Token::CloseParen)) {
                args.push(self.expression());
                if let Some(&Token::Comma) = self.peek() {
                    self.eat();
                }
            }
            devout!(self "arguments");

            expect_token!(
                self,
                CloseParen,
                SiltError::UnterminatedParenthesis(start.0, start.1)
            );

            Ok(args)
        }
        fn primary(&mut self) -> Expression {
            // Err(code) => {
            //     println!("Error Heere: {} :{}", code, self.current);
            //     self.error(code);
            //     Expression::InvalidExpression
            // }
            devout!(self "primary");

            let t = self.next();
            let location = self.get_last_loc();
            // errors will 1 ahead, use error_last
            match t {
                Some(Token::Number(n)) => Expression::Literal {
                    value: Value::Number(n),
                    location,
                },
                Some(Token::StringLiteral(s)) => Expression::Literal {
                    value: Value::String(s),
                    location,
                },
                Some(Token::Integer(i)) => Expression::Literal {
                    value: Value::Integer(i),
                    location,
                },
                Some(Token::True) => Expression::Literal {
                    value: Value::Bool(true),
                    location,
                },
                Some(Token::False) => Expression::Literal {
                    value: Value::Bool(false),
                    location,
                },
                Some(Token::Nil) => Expression::Literal {
                    value: Value::Nil,
                    location,
                },

                Some(Token::OpenParen) => {
                    let start = self.get_last_loc(); // we're ahead normally, in this func we're ahead by 2 as we already ate, yummers
                    let exp = self.expression();
                    if let Some(Token::CloseParen) = self.peek() {
                        self.eat();
                        Expression::GroupingExpression {
                            expression: Box::new(exp),
                            location: start,
                        }
                    } else {
                        self.error(SiltError::UnterminatedParenthesis(start.0, start.1));
                        Expression::InvalidExpression
                    }
                }
                Some(Token::Identifier(ident)) => Expression::Variable {
                    ident: (self.global.to_register(&ident), 0),
                    location,
                },
                Some(Token::Function) => self.function_expression(location),
                // Some(Token::EOF) => Ok(Expression::EndOfFile),
                Some(Token::Op(o)) => {
                    self.error(SiltError::ExpInvalidOperator(o));
                    Expression::InvalidExpression
                }
                Some(tt) => {
                    // TODO nil?
                    self.error(SiltError::InvalidTokenPlacement(tt));
                    Expression::InvalidExpression
                }
                None => {
                    self.error_last(SiltError::EarlyEndOfFile);
                    Expression::InvalidExpression
                }
            }
        }

        fn de_op(t: Token) -> Operator {
            if let Token::Op(o) = t {
                return o;
            }
            panic!("Bad operator") // can this happen
        }
    }
}
