pub mod parser {
    use crate::{
        error::{ErrorTuple, ErrorTypes, Location, SiltError},
        expression::Expression,
        statement::Statement,
        token::{Operator, Token},
        value::Value,
    };

    // macro_rules! val_err {
    //     ($left:ident,$right:ident) => {
    //         return Err(SiltError::ExpAddValueWithValue(
    //             Value::String($left),
    //             Value::String($right),
    //         ));
    //     };
    // }
    macro_rules! op_assign {
        ($self:ident, $ident:ident,$op:ident) => {{
            let value = $self.next_expression();
            Statement::Declare {
                ident: $ident.clone(),
                value: Expression::Binary {
                    left: Box::new(Expression::Variable { $ident }),
                    operator: Operator::$op,
                    right: Box::new(value),
                },
            }
        }};
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
            assert!(p.len() == p.len());
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

        pub fn eat(&mut self) {
            self.current += 1;
            self.iterator.next();
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
            // println!(" eat out pos {}", self.current);
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

        pub fn parse(&mut self) -> Vec<Statement> {
            let mut statements = vec![];
            while !self.is_end() {
                // if let Ok(s) = self.statement() {
                statements.push(self.local_declaration());
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

        fn local_declaration(&mut self) -> Statement {
            if let Some(&Token::Local) = self.peek() {
                self.eat();
                self.declaration(true)
            } else {
                self.declaration(false)
            }
        }

        fn declaration(&mut self, local: bool) -> Statement {
            // if let Some(&Token::Local) = self.peek() {
            //     self.eat();

            if matches!(self.peek(), Some(Token::Identifier(_))) {
                if let Token::Identifier(ident) = self.eat_out() {
                    if let Some(&Token::Colon) = self.peek() {
                        // typing or self calling
                        self.eat();
                        let token = self.eat_out();
                        if let Token::ColonIdentifier(target) = token {
                            // method or type name
                            // self.eat();
                            // return self.assign(self.peek(), ident);
                            if let Some(&Token::OpenParen) = self.peek() {
                                // self call

                                Statement::InvalidStatement
                            } else {
                                // typing
                                // return self.assign(self.peek(), ident);
                                Statement::InvalidStatement
                            }
                        } else {
                            self.error(SiltError::InvalidColonPlacement);
                            Statement::InvalidStatement
                        }
                    } else {
                        return if local {
                            self.local_declare(ident)
                        } else {
                            self.assign(ident)
                        };
                    }
                } else {
                    // yucky
                    panic!("impossible");
                }
            } else if local {
                self.error(SiltError::ExpectedLocalIdentifier);
                Statement::InvalidStatement
            } else {
                self.statement()
            }
        }

        fn assignment(&mut self) -> Expression {
            let exp = self.equality();
            if let Some(&Token::Assign) = self.peek() {
                // println!("assign");
                //let ident= current somehow?? use the exp as ident?
                if let Expression::Variable { ident } = exp {
                    self.eat();
                    let value = self.assignment();
                    return Expression::Assign {
                        ident,
                        value: Box::new(value),
                    };
                }
                // self.eat();

                // let value = self.equality();
                // return Expression::Assign {
                //     ident,
                //     value: Box::new(value),
                // };
            }
            exp
        }

        fn local_declare(&mut self, ident: String) -> Statement {
            let t = self.peek();
            // println!("decl");
            match t {
                Some(&Token::Assign) => Statement::Declare {
                    ident,
                    value: self.next_expression(),
                },
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
                    value: Expression::Literal { value: Value::Nil },
                },
            }
        }

        fn assign(&mut self, ident: String) -> Statement {
            match self.peek() {
                Some(&Token::Assign) => Statement::Declare {
                    ident,
                    value: self.next_expression(),
                },
                Some(&Token::AddAssign) => {
                    op_assign!(self, ident, Add)
                }
                Some(&Token::SubAssign) => {
                    op_assign!(self, ident, Sub)
                }
                Some(&Token::MultiplyAssign) => {
                    op_assign!(self, ident, Multiply)
                }
                Some(&Token::DivideAssign) => {
                    op_assign!(self, ident, Divide)
                }
                Some(&Token::ModulusAssign) => {
                    op_assign!(self, ident, Modulus)
                }
                _ => Statement::Declare {
                    ident,
                    value: Expression::Literal { value: Value::Nil },
                },
            }
        }

        fn statement(&mut self) -> Statement {
            match self.peek() {
                Some(&Token::Print) => Statement::Print(self.next_expression()),
                Some(&Token::Do) => Statement::Block(self.block()),
                _ => Statement::Expression(self.expression()), // don't eat
            }
        }

        fn block(&mut self) -> Vec<Statement> {
            self.eat();
            let mut statements = vec![];
            while !matches!(self.peek(), Some(&Token::End)) {
                self.eat();
                statements.push(self.statement());
            }

            if !matches!(self.eat_out(), Token::End) {
                self.error(SiltError::UnterminatedBlock);
            }
            statements
        }

        fn next_expression(&mut self) -> Expression {
            self.eat();
            self.expression()
        }

        fn expression(&mut self) -> Expression {
            self.assignment()
        }

        fn equality(&mut self) -> Expression {
            let mut exp = self.comparison();
            while let Some(&Token::Op((Operator::NotEqual | Operator::Equal))) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.comparison();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }

        fn comparison(&mut self) -> Expression {
            let mut exp = self.term();

            while let Some(&Token::Op(
                Operator::LessThan
                | Operator::LessThanOrEqual
                | Operator::GreaterThan
                | Operator::GreaterThanOrEqual,
            )) = self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let right = self.term();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }

        fn term(&mut self) -> Expression {
            let mut exp = self.factor();
            while let Some(&Token::Op(Operator::Add | Operator::Sub)) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.factor();
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
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
                exp = Expression::Binary {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }

        // op unary or primary
        pub fn unary(&mut self) -> Expression {
            if let Some(&Token::Op(Operator::Sub | Operator::Not | Operator::Tilde)) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.unary();
                Expression::Unary {
                    operator,
                    right: Box::new(right),
                }
            } else {
                self.primary()
            }
        }

        fn primary(&mut self) -> Expression {
            // Err(code) => {
            //     println!("Error Heere: {} :{}", code, self.current);
            //     self.error(code);
            //     Expression::InvalidExpression
            // }

            let t = self.next();
            // errors will 1 ahead, use error_last
            match t {
                Some(Token::Number(n)) => Expression::Literal {
                    value: Value::Number(n),
                },
                Some(Token::StringLiteral(s)) => Expression::Literal {
                    value: Value::String(s),
                },
                Some(Token::Integer(i)) => Expression::Literal {
                    value: Value::Integer(i),
                },
                Some(Token::True) => Expression::Literal {
                    value: Value::Bool(true),
                },
                Some(Token::False) => Expression::Literal {
                    value: Value::Bool(false),
                },
                Some(Token::Nil) => Expression::Literal { value: Value::Nil },

                Some(Token::OpenParen) => {
                    let start = self.locations[self.current - 1]; // we're ahead normally, in this func we're ahead by 2 as we already ate, yummers
                    let exp = self.expression();
                    if let Some(Token::CloseParen) = self.peek() {
                        self.eat();
                        Expression::GroupingExpression {
                            expression: Box::new(exp),
                        }
                    } else {
                        self.error(SiltError::UnterminatedParenthesis(start.0, start.1));
                        Expression::InvalidExpression
                    }
                }
                Some(Token::Identifier(ident)) => Expression::Variable { ident },
                // Some(Token::EOF) => Ok(Expression::EndOfFile),
                Some(Token::Op(o)) => {
                    self.error(SiltError::InvalidExpressionOperator(o));
                    Expression::InvalidExpression
                }
                Some(tt) => {
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
