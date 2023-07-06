pub mod compiler {
    use std::{
        f32::consts::E,
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
            println!(
                "=> {}: {:?} -> {:?}",
                $message,
                $self.peek().unwrap_or(&Token::Nil).clone(),
                $self.get_current().unwrap_or(&Token::Nil)
            );
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

    /** the higher the precedence, */
    #[derive(PartialEq, PartialOrd)]
    enum Precedence {
        None,
        Assignment, // =
        Or,         // or
        And,        // and
        Equality,   // == ~= !=
        Comparison, // < > <= >=
        Term,       // + - ..
        Factor,     // * /
        Unary,      // ~ - !
        Call,       // . ()
        Primary,
    }
    // precedence enum includes concat ..

    type Ident = u8;

    type Catch = Result<(), ErrorTuple>;

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
        prefix: fn(&mut Compiler, can_assign: bool) -> Catch,
        infix: fn(&mut Compiler, can_assign: bool) -> Catch,
        precedence: Precedence,
    }

    pub struct Compiler {
        chunk: Chunk,
        iterator: Peekable<Lexer>,
        pub current_index: usize,
        pub errors: Vec<ErrorTuple>,
        pub valid: bool,
        current: Result<Token, ErrorTuple>,
        previous: Result<Token, ErrorTuple>,
        current_location: Location,
        // location: (usize, usize),
        // previous: TokenTuple,
        // pre_previous: TokenTuple,
        extra: bool, // pub global: &'a mut Environment,
    }
    impl<'a> Compiler {
        pub fn new() -> Compiler {
            // assert!(p.len() == p.len());
            Self {
                chunk: Chunk::new(),
                iterator: Lexer::new("".to_string()).peekable(),
                current: Ok(Token::Nil),
                previous: Ok(Token::Nil),
                current_location: (0, 0),
                current_index: 0,
                errors: vec![],
                valid: true,
                // location: (0, 0),
                // previous: (Token::Nil, (0, 0)),
                // pre_previous: (Token::Nil, (0, 0)),
                extra: true,
            }
        }

        fn error_syntax(&mut self, code: SiltError, location: Location) -> ErrorTuple {
            self.valid = false;
            self.chunk.invalidate();
            ErrorTuple { code, location }
        }

        fn error_at(&mut self, code: SiltError) -> ErrorTuple {
            self.error_syntax(code, self.current_location)
        }
        pub fn print_errors(&self) {
            for e in &self.errors {
                println!("!!{} at {}:{}", e.code, e.location.0, e.location.1);
            }
        }
        fn push_error(&mut self, code: ErrorTuple) {
            self.errors.push(code);
        }

        pub fn get_errors(&self) -> &Vec<ErrorTuple> {
            &self.errors
        }

        /** pop and return the token tuple, take care as this does not wipe the current token */
        pub fn pop(&mut self) -> (Result<Token, ErrorTuple>, Location) {
            self.current_index += 1;
            match self.iterator.next() {
                Some(Ok(t)) => {
                    #[cfg(feature = "dev-out")]
                    println!("popped {}", t.0);
                    (Ok(t.0), t.1)
                }
                Some(Err(e)) => {
                    let l = e.location;
                    (Err(e), l)
                }
                None => (Ok(Token::EOF), (0, 0)),
            }
        }

        /** Slightly faster pop that devourse the token or error, should follow a peek or risk skipping as possible error. Probably irrelevant otherwise. */
        pub fn eat(&mut self) {
            self.current_index += 1;
            let t = self.iterator.next();
            #[cfg(feature = "dev-out")]
            {
                match t {
                    Some(Ok(t)) => println!("eat {}", t.0),
                    Some(Err(e)) => println!("eat {}", e.code),
                    None => println!("eat {:?}", Token::EOF),
                }
            }
        }

        /** pop and store on to self as current token tuple */
        pub fn store(&mut self) {
            self.current_index += 1;
            (self.current, self.current_location) = match self.iterator.next() {
                Some(Ok(t)) => (Ok(t.0), t.1),
                Some(Err(e)) => {
                    // self.error_syntax(e.code, e.location);
                    let l = e.location;
                    (Err(e), l)
                }
                None => (Ok(Token::EOF), (0, 0)),
            };
        }

        pub fn take_store(&mut self) -> Result<Token, ErrorTuple> {
            // std::mem::replace(&mut self.current, Ok(Token::Nil))
            self.current.clone()
        }

        /** pop and store on to self as current token tuple, return the tuple we replaced */
        fn store_and_return(&mut self) -> Result<Token, ErrorTuple> {
            self.current_index += 1;
            let (r, l) = match self.iterator.next() {
                Some(Ok(t)) => (Ok(t.0), t.1),
                Some(Err(e)) => {
                    // self.error_syntax(e.code, e.location);
                    let l = e.location;
                    (Err(e), l)
                }
                None => (Ok(Token::EOF), (0, 0)),
            };

            self.current_location = l;
            std::mem::replace(&mut self.current, r)
        }

        pub fn get_current(&self) -> Result<&Token, ErrorTuple> {
            match &self.current {
                Ok(t) => {
                    #[cfg(feature = "dev-out")]
                    println!("get_current {}", t);
                    Ok(t)
                }
                Err(e) => Err(e.clone()),
            }
        }

        /** only use after peek */
        // pub fn eat_out(&mut self) -> TokenResult {
        //     self.current_index += 1;
        //     self.iterator.next().unwrap()
        // }

        fn peek(&mut self) -> Result<&Token, ErrorTuple> {
            match self.iterator.peek() {
                Some(Ok(t)) => {
                    #[cfg(feature = "dev-out")]
                    println!("peek {}", t.0);
                    Ok(&t.0)
                }
                Some(Err(e)) => {
                    // self.error_syntax(e.code, e.location);
                    #[cfg(feature = "dev-out")]
                    println!("peek err {}", e.code);
                    let l = e.location;
                    Err(ErrorTuple {
                        code: e.code.clone(),
                        location: l,
                    })
                }
                None => Ok(&Token::EOF),
            }
        }

        fn peek_result(&mut self) -> &TokenResult {
            #[cfg(feature = "dev-out")]
            match self.iterator.peek() {
                Some(r) => {
                    match r {
                        Ok(t) => {
                            println!("peek_res {}", t.0);
                        }
                        Err(e) => {
                            println!("peek_res err {}", e.code);
                        }
                    }
                    r
                }
                None => &Ok((Token::EOF, (0, 0))),
            }
            #[cfg(not(feature = "dev-out"))]
            match self.iterator.peek() {
                Some(r) => r,
                None => &Ok((Token::EOF, (0, 0))),
            }
        }

        fn emit(&mut self, op: OpCode, location: Location) {
            self.chunk.write_code(op, location);
            #[cfg(feature = "dev-out")]
            {
                println!("emit ...");
                self.chunk.print_chunk();
            }
        }

        fn emit_at(&mut self, op: OpCode) {
            self.chunk.write_code(op, self.current_location);
            #[cfg(feature = "dev-out")]
            {
                println!("emit_at ...");
                self.chunk.print_chunk();
            }
        }

        fn identifer_constant(&mut self, ident: Box<String>) -> u8 {
            self.chunk.write_constant(Value::String(ident)) as u8
        }

        fn constant(&mut self, value: Value, location: Location) {
            let constant = self.chunk.write_constant(value) as u8;
            self.emit(OpCode::CONSTANT { constant }, location);
        }
        fn constant_at(&mut self, value: Value) {
            self.constant(value, self.current_location);
        }

        fn current_chunk(&self) -> &Chunk {
            &self.chunk
        }

        fn is_end(&mut self) -> bool {
            match self.iterator.peek() {
                None => true,
                _ => false,
            }
        }

        // fn get_prev_loc(&self) -> Location {
        //     #[cfg(feature = "dev-out")]
        //     println!(
        //         "get index {} is loc {}:{}",
        //         self.current, self.locations[self.current].0, self.locations[self.current].1
        //     );
        //     // self.locations[self.current]
        //     // TODO current?
        //     self.previous.1
        // }

        // fn get_pre_prev_loc(&self) -> Location {
        //     #[cfg(feature = "dev-out")]
        //     println!(
        //         "get index {} is loc {}:{}",
        //         self.current - 1,
        //         self.locations[self.current - 1].0,
        //         self.locations[self.current - 1].1
        //     );
        //     self.pre_previous.1
        // }

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
                    Operator::Concat => rule!(void, concat, Term),
                    _ => rule!(void, void, None),
                },
                Token::Identifier(_) => rule!(variable, void, None),
                Token::Integer(_) => rule!(integer, void, None),
                Token::StringLiteral(_) => rule!(string, void, None),
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
            // self.eat();
            // self.store();

            while !self.is_end() {
                match declaration(self) {
                    Ok(()) => {}
                    Err(e) => {
                        self.push_error(e);
                        self.synchronize();
                    }
                }
            }

            // while !self.is_end() {

            // }

            self.emit(OpCode::RETURN, (0, 0));

            std::mem::take(&mut self.chunk).into()
        }
        fn synchronize(&mut self) {
            // TODO should we unwind or just dump it all?
            // self.eat();
            // while !self.is_end() {
            //     match self.get_current() {
            //         Ok(Token::Print) => return,
            //         _ => {}
            //     }
            //     self.eat();
            // }
        }

        fn parse_precedence(&mut self, precedence: Precedence, skip_step: bool) -> Catch {
            if !skip_step {
                self.store();
            }
            // self.store(); // MARK with store first it works for normal statements, but it breaks for incomplete expressions that are meant to pop off
            // Basically the integer we just saw is dropped off when we reach here because of store
            let t = self.get_current()?;
            let loc = self.current_location;

            println!("check rule for token {}", t);
            let rule = Self::get_rule(&t);
            // #[cfg(feature = "dev-out")]
            println!(
                "target precedence:  {}, current precedence: {}",
                precedence, rule.precedence
            );
            // if (rule.prefix) != Self::void { // TODO bubble error up if no prefix, call invalid func to bubble?
            let can_assign = precedence <= Precedence::Assignment;
            (rule.prefix)(self, can_assign)?;
            // self.store();
            // let c = &self.current?.0;

            loop {
                let c = self.peek_result();
                let rule = match c {
                    Ok((Token::EOF, _)) => break,
                    Ok((t, _)) => Self::get_rule(t),
                    Err(e) => {
                        return Err(e.clone());
                    }
                };
                // let c = self.get_current()?;
                // let rule = Self::get_rule(c);
                // #[cfg(feature = "dev-out")]
                println!(
                    "loop target precedence for :  {}, current precedence for  : {}",
                    precedence, rule.precedence
                );
                if precedence > rule.precedence {
                    break;
                }
                self.store();
                (rule.infix)(self, false)?;
            }

            // TODO test this with `local b="b" sprint b`
            if can_assign
                && if let Token::Assign = self.peek()? {
                    true
                } else {
                    false
                }
            {
                let res = self.peek()?.clone();
                return Err(self.error_at(SiltError::InvalidAssignment(res)));
            }

            // while precedence <= Self::get_rule(&self.current?.0).precedence {
            //     self.store_and_return()
            //     let rule = Self::get_rule(&self.current?.0);
            //     (rule.infix)(self);
            // }
            // self.store();
            if skip_step {
                self.store();
            }
            Ok(())
        }
    }

    fn declaration(this: &mut Compiler) -> Catch {
        devout!(this "declaration");

        let t = this.peek()?;
        match t {
            Token::Local => declaration_scope(this, true, false)?,
            Token::Global => declaration_scope(this, false, false)?,
            // Token::Function => {
            //     this.eat();
            //     this.define_function(true, None)
            // }
            _ => statement(this)?,
        }
        Ok(())
    }

    fn declaration_scope(this: &mut Compiler, local: bool, already_function: bool) -> Catch {
        devout!(this "declaration_scope");
        this.eat();
        // this.advance();
        // this.store();
        let (res, location) = this.pop();
        match res? {
            Token::Identifier(ident) => {
                // MARK
                let ident = this.identifer_constant(ident); //(self.global.to_register(&ident), 0);
                typing(this, ident, location, local)?;
            }
            // Token::Function => {
            //     if !already_function {
            //         self.define_function(local, None)
            //     } else {
            //         self.error(SiltError::ExpectedLocalIdentifier);
            //         Statement::InvalidStatement
            //     }
            // }
            // _ => {
            //     self.error(SiltError::ExpectedLocalIdentifier);
            //     Statement::InvalidStatement
            // }
            _ => todo!(),
        }
        Ok(())
    }

    fn typing(this: &mut Compiler, ident: Ident, location: Location, local: bool) -> Catch {
        devout!(this "typing");
        if let Token::Colon = this.peek()? {
            // typing or self calling
            this.eat();
            this.store();
            let t = this.get_current()?;
            if let Token::ColonIdentifier(target) = t {
                // method or type name
                // if let Some(&Token::OpenParen) = self.peek() {
                //     // self call

                //     Statement::InvalidStatement
                // } else {
                //     // typing
                //     // return self.assign(self.peek(), ident);
                //     Statement::InvalidStatement
                // }
                define_declaration(this, ident, location, local)?;
            } else {
                todo!("typing");
                // self.error(SiltError::InvalidColonPlacement);
                // Statement::InvalidStatement
            }
        } else {
            define_declaration(this, ident, location, local)?;
        }
        Ok(())
    }

    fn define_declaration(
        this: &mut Compiler,
        ident: Ident,
        location: Location,
        local: bool,
    ) -> Catch {
        devout!(this "define_declaration");
        this.store();
        let t = this.get_current()?;
        match t {
            Token::Assign => {
                expression(this, false)?;
            }
            // we can't increment what doesn't exist yet, like what are you even doing?
            Token::AddAssign
            | Token::SubAssign
            | Token::MultiplyAssign
            | Token::DivideAssign
            | Token::ModulusAssign => {
                // let tt = t.unwrap().clone(); // TODO
                // self.error(SiltError::InvalidAssignment(tt));
                // Statement::InvalidStatement
            }
            _ => this.emit_at(OpCode::NIL),
        }
        define_variable(this, ident, location, local)?;
        Ok(())
    }

    fn define_variable(
        this: &mut Compiler,
        ident: Ident,
        location: Location,
        local: bool,
    ) -> Catch {
        devout!(this "define_variable");
        if local {
            this.emit(OpCode::DEFINE_GLOBAL { constant: ident }, location); // TODO
        } else {
            this.emit(OpCode::DEFINE_GLOBAL { constant: ident }, location);
        }
        Ok(())
    }

    fn statement(this: &mut Compiler) -> Catch {
        devout!(this "statement");
        match this.peek()? {
            Token::Print => print(this)?,
            // Token::OpenBrace => block(this),
            _ => expression_statement(this)?,
        }
        Ok(())
    }

    fn expression(this: &mut Compiler, skip_step: bool) -> Catch {
        devout!(this "expression");
        this.parse_precedence(Precedence::Assignment, skip_step)?;
        Ok(())
    }

    fn next_expression(this: &mut Compiler) -> Catch {
        devout!(this "next_expression");
        this.eat();
        expression(this, false)?;
        Ok(())
    }

    fn expression_statement(this: &mut Compiler) -> Catch {
        devout!(this "expression_statement");
        expression(this, false)?;
        this.emit_at(OpCode::POP);
        devout!(this "expression_statement end");
        Ok(())
    }

    fn variable(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "variable");
        // let t = this.previous.clone();
        // let ident = if let Token::Identifier(ident) = t.0 {
        //     this.identifer_constant(ident)
        // } else {
        //     unreachable!()
        // };
        // if let Some(Ok((Token::Assign, _))) = this.peek() {
        //     this.advance();
        //     next_expression(this);
        //     this.emit(OpCode::DEFINE_GLOBAL { constant: ident }, t.1);
        // } else {
        //     this.emit(OpCode::LITERAL { dest: ident, literal: ident }, t.1);
        // }
        named_variable(this, can_assign, false)?;
        Ok(())
    }

    fn named_variable(this: &mut Compiler, can_assign: bool, local: bool) -> Catch {
        devout!(this "named_variables");
        let t = this.take_store()?;
        let ident = if let Token::Identifier(ident) = t {
            println!("assigning to identifier: {}", ident);
            this.identifer_constant(ident)
        } else {
            unreachable!()
        };

        if can_assign
            && if let Token::Assign = this.peek()? {
                true
            } else {
                false
            }
        {
            this.eat();
            let loc = this.current_location;
            expression(this, false)?;
            if local {
                // TODO local
                this.emit(OpCode::SET_GLOBAL { constant: ident }, loc);
            } else {
                this.emit(OpCode::SET_GLOBAL { constant: ident }, loc);
            }
        } else {
            this.emit_at(OpCode::GET_GLOBAL { constant: ident });
        }

        // if let &Token::Assign = this.get_current()? {
        //     let loc = this.current_location;
        //     expression(this, false)?;
        //     this.emit(OpCode::DEFINE_GLOBAL { constant: ident }, loc);
        // } else {
        //     this.emit_at(OpCode::GET_GLOBAL { constant: ident });
        // }
        Ok(())
    }

    fn grouping(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "grouping");
        expression(this, false)?;
        //TODO expect
        // expect_token!(self, CloseParen, SiltError::UnterminatedParenthesis(0, 0));
        // self.consume(TokenType::RightParen, "Expect ')' after expression.");
        Ok(())
    }

    /** op unary or primary */
    fn unary(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "unary");
        let t = this.take_store()?;
        // self.expression();

        this.parse_precedence(Precedence::Unary, false)?;
        match t {
            Token::Op(Operator::Sub) => this.emit_at(OpCode::NEGATE),
            Token::Op(Operator::Not) => this.emit_at(OpCode::NOT),
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
        Ok(())
    }

    fn binary(this: &mut Compiler, _can_assign: bool) -> Catch {
        devout!(this "binary");
        let t = this.take_store()?;
        let l = this.current_location;
        let rule = Compiler::get_rule(&t);
        this.parse_precedence(rule.precedence.next(), false)?;
        if let Token::Op(op) = t {
            match op {
                Operator::Add => this.emit(OpCode::ADD, l),
                Operator::Sub => this.emit(OpCode::SUB, l),
                Operator::Multiply => this.emit(OpCode::MULTIPLY, l),
                Operator::Divide => this.emit(OpCode::DIVIDE, l),

                Operator::Concat => this.emit(OpCode::CONCAT, l),

                // Operator::Modulus => self.emit(OpCode::MODULUS, t.1),
                // Operator::Equal => self.emit(OpCode::EQUAL, t.1),
                Operator::Equal => this.emit(OpCode::EQUAL, l),
                Operator::NotEqual => this.emit(OpCode::NOT_EQUAL, l),
                Operator::Less => this.emit(OpCode::LESS, l),
                Operator::LessEqual => this.emit(OpCode::LESS_EQUAL, l),
                Operator::Greater => this.emit(OpCode::GREATER, l),
                Operator::GreaterEqual => this.emit(OpCode::GREATER_EQUAL, l),

                _ => todo!(),
            }
        }
        Ok(())
    }

    fn concat(this: &mut Compiler, _can_assign: bool) -> Catch {
        devout!(this "concat_binary");
        let t = this.take_store()?;
        let l = this.current_location;
        let rule = Compiler::get_rule(&t);
        this.parse_precedence(rule.precedence.next(), false)?;

        if let Token::Op(op) = t {
            match op {
                Operator::Concat => this.emit(OpCode::CONCAT, l),
                _ => todo!(),
            }
        }
        Ok(())
    }

    fn integer(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "integer");
        let t = this.take_store()?;
        let value = if let Token::Integer(i) = t {
            println!("integer: {}", i);
            Value::Integer(i)
        } else {
            unreachable!()
        };
        this.constant_at(value);
        Ok(())
    }
    fn string(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "string");
        let t = this.take_store()?;
        let value = if let Token::StringLiteral(s) = t {
            println!("string: {}", s);
            Value::String(Box::new(s.into_string()))
        } else {
            unreachable!()
        };
        this.constant_at(value);
        Ok(())
    }

    fn literal(this: &mut Compiler, can_assign: bool) -> Catch {
        devout!(this "literal");
        let t = this.take_store()?;
        match t {
            Token::Nil => this.emit_at(OpCode::NIL),
            Token::True => this.emit_at(OpCode::TRUE),
            Token::False => this.emit_at(OpCode::FALSE),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn print(this: &mut Compiler) -> Catch {
        devout!(this "print");
        this.eat();
        expression(this, false)?;
        this.emit_at(OpCode::PRINT);
        Ok(())
    }

    pub fn void(_this: &mut Compiler, _can_assign: bool) -> Catch {
        devout!(_this "void");
        Ok(())
    }

    // pub fn invalid(_: &mut Compiler) { // TODO
    //                                    // this.error(SiltError::InvalidExpression);
    // }
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
