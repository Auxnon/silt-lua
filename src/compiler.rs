use std::{
    fmt::{Display, Formatter},
    iter::Peekable,
    mem::{swap, take},
    println,
    rc::Rc,
    vec,
};

use hashbrown::HashMap;

use crate::{
    chunk::Chunk,
    code::OpCode,
    error::{ErrorTuple, Location, SiltError},
    function::FunctionObject,
    lexer::{Lexer, TokenResult},
    token::{Operator, Token},
    value::Value,
};

macro_rules! build_block_until {
        ($self:ident, $($rule:ident)|*) => {{
            while match $self.peek()? {
                $( Token::$rule)|* => {$self.eat(); false}
                Token::EOF => {
                    return Err($self.error_at(SiltError::UnterminatedBlock));
                }
                _ =>{declaration($self)?; true}
            } {
            }
        }};
    }

macro_rules! devnote {
    ($self:ident $message:literal) => {
        #[cfg(feature = "dev-out")]
        println!(
            "=> {}: peek: {:?} -> current: {:?}",
            $message,
            $self.peek().unwrap_or(&Token::Nil).clone(),
            $self.get_current().unwrap_or(&Token::Nil)
        );
    };
}

macro_rules! devout {
        ($($arg:tt)*) => {
            #[cfg(feature = "dev-out")]
            println!($($arg)*);
        }
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
        if let Token::$token = $self.peek()? {
            $self.eat();
        } else {
            return Err($self.error_at(SiltError::ExpectedToken(Token::$token)));
        }
    };};
    ($self:ident, $token:ident, $custom_error:expr) => {{
        if let Token::$token = $self.peek()? {
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
    Concat,     // ..
    Term,       // + -
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
            Precedence::Comparison => Precedence::Concat,
            Precedence::Concat => Precedence::Term,
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
            Precedence::Concat => write!(f, "Concat"),
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

/** stores a local identifier's name by boxed string, if none is provbided it serves as a placeholder for statements such as a loop, this way they cannot be resolved as variables */
struct Local {
    ident: Option<Box<String>>,
    /** scope depth, indepedent of functional depth */
    depth: usize,
    /** how many layers deep the local value is nested in a function, with 0 being global (should only happen once to reserve the root func on the stack) */
    functional_depth: usize,
    is_captured: bool,
}

struct UpLocal {
    /** location on the overall stack */
    ident: u8,
    /** location on the immediately scoped stack, 1 if it's the first declared value in scope (after closure) */
    // scoped_ident: u8,
    neighboring: bool,
    universal_ident: u8,
}

pub struct Compiler {
    pub body: FunctionObject,
    iterator: Peekable<Lexer>,
    pub current_index: usize,
    pub errors: Vec<ErrorTuple>,
    pub valid: bool,
    current: Result<Token, ErrorTuple>,
    current_location: Location,
    local_count: usize,
    scope_depth: usize,
    functional_depth: usize,
    // TODO we need a fail catch if we exceed a local variable amount of up values as well
    up_values: Vec<Vec<UpLocal>>,
    /** an offset tracker each time we descend into a new functional scope. For instance if we drop 1 level down from the root level that had 3 locals prior [A,B,C] then our stack looks like [root, A, B, C, fn] then we'll store 3 at this field's index 0 since the calling function is always at the bottom of the stack */
    local_functional_offset: Vec<usize>,
    // locals: Vec<Local>,
    locals: Vec<Local>,
    labels: HashMap<String, usize>,
    // location: (usize, usize),
    // previous: TokenTuple,
    // pre_previous: TokenTuple,
    pending_gotos: Vec<(String, usize, Location)>,
    extra: bool, // pub global: &'a mut Environment,
    /** hack to flip off a pop when an expression takes on statement properties, used only for := right now */
    override_pop: bool,
}

impl<'a> Compiler {
    /** Create a new compiler instance */
    pub fn new() -> Compiler {
        // assert!(p.len() == p.len());
        Self {
            body: FunctionObject::new(None, true),
            iterator: Lexer::new("".to_string()).peekable(),
            current: Ok(Token::Nil),
            current_location: (0, 0),
            current_index: 0,
            errors: vec![],
            valid: true,
            local_count: 0,
            scope_depth: 0,
            functional_depth: 0,
            up_values: vec![vec![]],
            locals: vec![Local {
                ident: None,
                depth: 0,
                functional_depth: 0,
                is_captured: false,
            }],
            local_functional_offset: vec![],
            labels: HashMap::new(),
            pending_gotos: vec![],
            // location: (0, 0),
            // previous: (Token::Nil, (0, 0)),
            // pre_previous: (Token::Nil, (0, 0)),
            extra: true,
            override_pop: false,
        }
    }

    /** Syntax error with code at location */
    fn error_syntax(&mut self, code: SiltError, location: Location) -> ErrorTuple {
        self.valid = false;
        self.body.chunk.invalidate();
        ErrorTuple { code, location }
    }

    /**syntax error at current token location with provided code */
    fn error_at(&mut self, code: SiltError) -> ErrorTuple {
        self.error_syntax(code, self.current_location)
    }

    /** Print all syntax errors */
    pub fn print_errors(&self) {
        for e in &self.errors {
            println!("!!{} at {}:{}", e.code, e.location.0, e.location.1);
        }
    }

    pub fn error_string(&self) -> String {
        let mut s = String::new();
        for e in &self.errors {
            s.push_str(&format!(
                "!!{} at {}:{}",
                e.code, e.location.0, e.location.1
            ));
        }
        s
    }

    /** Push error and location on to error stack */
    fn push_error(&mut self, code: ErrorTuple) {
        self.errors.push(code);
    }

    /** Return current array of errors */
    pub fn get_errors(&self) -> &Vec<ErrorTuple> {
        &self.errors
    }

    pub fn pop_errors(&mut self) -> Vec<ErrorTuple> {
        std::mem::replace(&mut self.errors, vec![])
    }

    fn get_chunk(&self) -> &Chunk {
        &self.body.chunk
    }

    fn get_chunk_mut(&mut self) -> &mut Chunk {
        &mut self.body.chunk
    }

    fn get_chunk_size(&self) -> usize {
        self.body.chunk.code.len()
    }

    fn write_code(&mut self, byte: OpCode, location: Location) -> usize {
        self.body.chunk.write_code(byte, location)
    }

    fn read_last_code(&self) -> &OpCode {
        self.body.chunk.read_last_code()
    }

    fn write_identifier(&mut self, identifier: Box<String>) -> usize {
        self.body.chunk.write_identifier(identifier)
    }

    /** pop and return the token tuple, take care as this does not wipe the current token but does advance the iterator */
    fn pop(&mut self) -> (Result<Token, ErrorTuple>, Location) {
        self.current_index += 1;
        match self.iterator.next() {
            Some(Ok(t)) => {
                devout!("popped {}", t.0);
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
    fn eat(&mut self) {
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
    fn store(&mut self) {
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

    /** take by value and gain ownership of the currently stored token */
    fn copy_store(&mut self) -> Result<Token, ErrorTuple> {
        // std::mem::replace(&mut self.current, Ok(Token::Nil))
        self.current.clone()
    }

    /** take current, replace with next. a true pop*/
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

    /** return the current token result */
    fn get_current(&self) -> Result<&Token, ErrorTuple> {
        match &self.current {
            Ok(t) => {
                devout!("get_current {}", t);
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

    /** return the peeked token result */
    fn peek(&mut self) -> Result<&Token, ErrorTuple> {
        match self.iterator.peek() {
            Some(Ok(t)) => {
                devout!("peek {}", t.0);
                Ok(&t.0)
            }
            Some(Err(e)) => {
                // self.error_syntax(e.code, e.location);
                devout!("peek err {}", e.code);
                let l = e.location;
                Err(ErrorTuple {
                    code: e.code.clone(),
                    location: l,
                })
            }
            None => Ok(&Token::EOF),
        }
    }

    /** return the peeked token tuple (token+location) result */
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

    /** emit op code at token location */
    fn emit(&mut self, op: OpCode, location: Location) {
        #[cfg(feature = "dev-out")]
        {
            println!("emit ... {}", op);
            // self.chunk.print_chunk();
        }
        self.write_code(op, location);
    }

    /** emit op code at current token location */
    fn emit_at(&mut self, op: OpCode) {
        #[cfg(feature = "dev-out")]
        {
            println!("emit_at ...{}", op);
            // self.chunk.print_chunk()
        }
        self.write_code(op, self.current_location);
    }

    /** emit op code at current token location and return op index */
    fn emit_index(&mut self, op: OpCode) -> usize {
        #[cfg(feature = "dev-out")]
        {
            println!("emit_index ... {}", op);
            // frame.chunk.print_chunk()
        }
        self.write_code(op, self.current_location)
    }

    /** patch the op code that specified index */
    fn patch(&mut self, offset: usize) -> Catch {
        let jump = self.get_chunk().code.len() - offset - 1;
        if jump > u16::MAX as usize {
            self.error_at(SiltError::TooManyOperations);
        }
        // self.chunk.code[offset] = ((jump >> 8) & 0xff) as u8;
        // self.chunk.code[offset + 1] = (jump & 0xff) as u8;
        match self.get_chunk().code[offset] {
            OpCode::GOTO_IF_FALSE(_) => {
                self.get_chunk_mut().code[offset] = OpCode::GOTO_IF_FALSE(jump as u16)
            }
            OpCode::GOTO_IF_TRUE(_) => {
                self.get_chunk_mut().code[offset] = OpCode::GOTO_IF_TRUE(jump as u16)
            }
            OpCode::FORWARD(_) => self.get_chunk_mut().code[offset] = OpCode::FORWARD(jump as u16),
            OpCode::REWIND(_) => self.get_chunk_mut().code[offset] = OpCode::REWIND(jump as u16),
            _ => {
                return Err(self.error_at(SiltError::ChunkCorrupt));
            }
        }
        Ok(())
    }

    fn emit_rewind(&mut self, start: usize) {
        // we base the jump off of the index we'll be at one we've written the rewind op below
        let jump = (self.get_chunk_size() + 1) - start;
        if jump > u16::MAX as usize {
            self.error_at(SiltError::TooManyOperations);
        }
        self.write_code(OpCode::REWIND(jump as u16), self.current_location);
    }

    fn set_label(&mut self, label: String) {
        self.labels.insert(label, self.get_chunk_size());
    }

    fn identifer_constant(&mut self, ident: Box<String>) -> u8 {
        self.write_identifier(ident) as u8
    }

    /** write to constant table  */
    fn write_constant(&mut self, value: Value) -> u8 {
        self.body.chunk.write_constant(value) as u8
    }

    /** write to constant table and emit the op code at location */
    fn constant(&mut self, value: Value, location: Location) {
        let constant = self.write_constant(value);
        self.emit(OpCode::CONSTANT { constant }, location);
    }

    /** write to constant table and emit the op code at the current location */
    fn constant_at(&mut self, value: Value) {
        self.constant(value, self.current_location);
    }

    /** write identifier to constant table, remove duplicates, and emit code */
    fn emit_identifer_constant_at(&mut self, ident: Box<String>) {
        let constant = self.write_identifier(ident) as u8;
        self.emit(OpCode::CONSTANT { constant }, self.current_location);
    }

    fn is_end(&mut self) -> bool {
        match self.iterator.peek() {
            None => true,
            _ => false,
        }
    }

    /** replaces the conents of func with the compilers body */
    fn swap_function(&mut self, mut func: &mut FunctionObject) {
        swap(&mut self.body, func);
        // func
    }

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

        match token {
            Token::OpenParen => rule!(grouping, call, Call),
            Token::OpenBrace => rule!(tabulate, call_table, None),
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
                Operator::Concat => rule!(void, concat, Concat),
                Operator::And => rule!(void, and, And),
                Operator::Or => rule!(void, or, Or),
                Operator::Length => rule!(unary, void, None),
                _ => rule!(void, void, None),
            },
            Token::Identifier(_) => rule!(variable, void, None),
            // Token::OpenBracket => rule!(void, indexer, Call),
            Token::Integer(_) => rule!(integer, void, None),
            Token::Number(_) => rule!(number, void, None),
            Token::StringLiteral(_) => rule!(string, call_string, None),
            Token::Nil => rule!(literal, void, None),
            Token::True => rule!(literal, void, None),
            Token::False => rule!(literal, void, None),
            // Token::Bang => rule!(unary, void, None),
            _ => rule!(void, void, None),
        }
    }

    pub fn compile(&mut self, source: String) -> FunctionObject {
        #[cfg(feature = "dev-out")]
        {
            let lexer = Lexer::new(source.to_owned());
            lexer.for_each(|r| match r {
                Ok(t) => {
                    println!("token {}", t.0);
                }
                Err(e) => println!("err {}", e),
            });
        }
        let lexer = Lexer::new(source.to_owned());
        self.iterator = lexer.peekable();
        while !self.is_end() {
            match declaration(self) {
                Ok(()) => {}
                Err(e) => {
                    self.push_error(e);
                    self.synchronize();
                }
            }
        }

        self.emit(OpCode::RETURN, (0, 0));
        take(&mut self.body)
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

        devout!("check rule for token {}", t);
        let rule = Self::get_rule(&t);
        // #[cfg(feature = "dev-out")]
        devout!(
            "target precedence:  {}, current precedence: {}",
            precedence,
            rule.precedence,
        );
        // if (rule.prefix) != Self::void { // TODO bubble error up if no prefix, call invalid func to bubble?
        let can_assign = precedence <= Precedence::Assignment;
        (rule.prefix)(self, can_assign)?;

        loop {
            let c = self.peek_result();
            let rule = match c {
                Ok((Token::EOF, _)) => break,
                Ok((t, _)) => Self::get_rule(t),
                Err(e) => {
                    return Err(e.clone());
                }
            };
            devout!(
                "loop target precedence for :  {}, current precedence for  : {}",
                precedence,
                rule.precedence
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

        // if skip_step {
        //     self.store();
        // }
        Ok(())
    }
}

fn declaration(this: &mut Compiler) -> Catch {
    devout!("----------------------------");
    devnote!(this "declaration");

    let t = this.peek()?;
    match t {
        Token::Local => declaration_keyword(this, true, false)?,
        Token::Global => declaration_keyword(this, false, false)?,
        Token::Function => {
            this.eat();
            define_function(this, true, None)?;
        }
        _ => statement(this)?,
    }
    Ok(())
}

fn declaration_keyword(this: &mut Compiler, local: bool, already_function: bool) -> Catch {
    devnote!(this "declaration_keyword");
    this.eat();
    let (res, location) = this.pop();
    match res? {
        Token::Identifier(ident) => {
            if this.scope_depth > 0 && local {
                //local
                //TODO should we warn? redefine_behavior(this,ident)?
                add_local(this, ident)?;
                typing(this, None)?;
            } else {
                let ident = this.identifer_constant(ident); //(self.global.to_register(&ident), 0);
                typing(this, Some((ident, location)))?;
            }
        }
        Token::Function => {
            if !already_function {
                define_function(this, local, None)?;
            } else {
                return Err(this.error_at(SiltError::ExpectedLocalIdentifier));
                // Statement::InvalidStatement
            }
        }
        // _ => {
        //     self.error(SiltError::ExpectedLocalIdentifier);
        //     Statement::InvalidStatement
        // }
        _ => todo!(),
    }
    Ok(())
}

fn declaration_scope(
    this: &mut Compiler,
    ident: Box<String>,
    local: bool,
    location: Location,
) -> Catch {
    if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this, ident)?;
        typing(this, None)?;
    } else {
        let ident = this.identifer_constant(ident);
        typing(this, Some((ident, location)))?;
    }
    Ok(())
}

// TODO Warning?
// fn redefine_behavior(this: &mut Compiler, ident: Box<String>) -> Catch {
//     // TODO depth !=-1 ?
//     for l in this.locals.iter().rev() {
//         if l.depth != -1 && l.depth < this.scope_depth {
//             return Ok(());
//         } else {
//             if l.ident == ident {
//                 return Err(this.error_at(SiltError::AlreadyDefined(ident)));
//             }
//         }
//     }
//     Ok(())
// }

/** Store location as a local to resolve getters with, the index pointing to the stack */
fn add_local(this: &mut Compiler, ident: Box<String>) -> Result<u8, ErrorTuple> {
    _add_local(this, Some(ident))
}

/** Store location on the stack with a placeholder that cannot be resolved as a variable, only reserves for operations */
fn add_local_placeholder(this: &mut Compiler) -> Result<u8, ErrorTuple> {
    _add_local(this, None)
}

fn _add_local(this: &mut Compiler, ident: Option<Box<String>>) -> Result<u8, ErrorTuple> {
    devnote!(this "add_local");
    if this.local_count == 255 {
        return Err(this.error_at(SiltError::TooManyLocals));
    }
    this.local_count += 1;
    this.locals.push(Local {
        ident,
        depth: this.scope_depth,
        functional_depth: this.functional_depth,
        is_captured: false,
    });
    let i = this.local_count - 1;
    Ok(i as u8)
}

// /** Remove a single reserved local */
// fn pop_local(this: &mut Compiler) {
//     devnote!(this "pop_local");
//     this.local_count -= 1;
//     this.locals.pop();
// }

fn resolve_local(this: &mut Compiler, ident: &Box<String>) -> Option<(u8, bool)> {
    devnote!(this "resolve_local");
    for (i, l) in this.locals.iter_mut().enumerate().rev() {
        if let Some(local_ident) = &l.ident {
            if local_ident == ident {
                #[cfg(feature = "dev-out")]
                println!("matched local {}->{} at {}", ident, local_ident, i);
                let ident_byte = i as u8;

                // first establish we're accessing a value by a closure, it exists outside this function
                let is_upvalue = l.functional_depth < this.functional_depth;
                return if is_upvalue {
                    (*l).is_captured = true;
                    let offset_ident = if l.functional_depth > 0 {
                        let offset = this.local_functional_offset[l.functional_depth - 1];
                        i - offset
                    } else {
                        i
                    } as u8;
                    // MARK we're passing in a target depth of 0, huh?? that's global isnt it? our upvals dont exist there
                    Some((
                        resolve_upvalue(
                            &mut this.up_values,
                            ident_byte,
                            offset_ident,
                            this.functional_depth,
                            l.functional_depth,
                        ),
                        is_upvalue,
                    ))
                } else {
                    let offset_ident = if this.functional_depth > 0 {
                        let offset = this.local_functional_offset[this.functional_depth - 1];
                        i - offset
                    } else {
                        i
                    } as u8;
                    Some((offset_ident as u8, false))
                };
            }
        }
    }
    None
}

// fn add_upvalue(this: &mut Compiler, ident: u8, neighboring: bool) -> u8 {
//     let m = this.up_values.last_mut().unwrap();
//     let i = m.len();
//     m.push(UpLocal { ident, neighboring });
//     i as u8
// }

// fn register_upvalue_at(this: &mut Compiler, ident: u8, neighboring: bool, level: usize) -> u8 {
//     let mut m = &mut this.up_values[level];
//     let i = m.len();
//     m.push(UpLocal { ident, neighboring });
//     i as u8
// }

/** check if upvalue is registered at this closest level and decend down until reach destination, registiner upvalues as we go if not already*/
fn resolve_upvalue(
    up_values: &mut Vec<Vec<UpLocal>>,
    ident: u8,
    scoped_ident: u8,
    level: usize,
    target: usize,
) -> u8 {
    let m = &mut up_values[level];
    for (u, i) in m.iter().enumerate() {
        if i.universal_ident == ident {
            return u as u8;
        }
    }
    // if level is equal to or no greater than target +1
    if level <= target + 1 {
        m.push(UpLocal {
            ident: scoped_ident,
            universal_ident: ident,
            neighboring: true,
        });
        (m.len() - 1) as u8
    } else {
        // drop(m);
        let higher = resolve_upvalue(up_values, ident, scoped_ident, level - 1, target);
        let m = &mut up_values[level];
        m.push(UpLocal {
            ident: higher,
            universal_ident: ident,
            neighboring: false,
        });
        (m.len() - 1) as u8
        // resolve_upvalue(up_values, ident, level - 1, target)
    }
}

// fn resolve_upvalue(this: &mut Compiler, ident: &Box<String>) -> Result<Option<u8>, ErrorTuple> {
//     devnote!(this "resolve_upvalue");
//     if this.scope_depth == 0 {
//         return Ok(None);
//     }
//     if let Some(local) = resolve_local(this, ident)? {
//         return Ok(Some(add_upvalue(this, local, true)?));
//     }
//     if let Some(upvalue) = this.body.upvalues.iter().position(|u| u.0 == ident) {
//         return Ok(Some(upvalue as u8));
//     }
//     Ok(None)
// }

fn typing(this: &mut Compiler, ident_tuple: Option<(Ident, Location)>) -> Catch {
    devnote!(this "typing");
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
            define_declaration(this, ident_tuple)?;
        } else {
            todo!("typing");
            // self.error(SiltError::InvalidColonPlacement);
            // Statement::InvalidStatement
        }
    } else {
        define_declaration(this, ident_tuple)?;
    }
    Ok(())
}

fn define_declaration(this: &mut Compiler, ident_tuple: Option<(Ident, Location)>) -> Catch {
    devnote!(this "define_declaration");
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
            todo!()
        }
        _ => this.emit_at(OpCode::NIL), // TODO are more then just declarations hitting this syntactic sugar?
    }
    define_variable(this, ident_tuple)?;
    Ok(())
}

fn define_variable(this: &mut Compiler, ident: Option<(Ident, Location)>) -> Catch {
    devnote!(this "define_variable");

    if let Some(ident) = ident {
        this.emit(OpCode::DEFINE_GLOBAL { constant: ident.0 }, ident.1);
    }
    Ok(())
}

fn define_function(this: &mut Compiler, local: bool, pre_ident: Option<usize>) -> Catch {
    let (ident, location) = if let &Token::Identifier(_) = this.peek()? {
        let (res, location) = this.pop();
        if let Token::Identifier(ident) = res? {
            (ident, location)
        } else {
            unreachable!()
        }
    } else {
        (Box::new("anonymous".to_string()), this.current_location)
    };

    let ident_clone = *ident.clone();
    let global_ident = if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this, ident)?;
        None
    } else {
        let ident = Some((this.identifer_constant(ident), location));
        ident
    };

    build_function(this, ident_clone, global_ident, false, false)?;

    Ok(())
}

/** builds function, implicit return specifices whether a nil is return or the last value popped */
fn build_function(
    this: &mut Compiler,
    ident: String,
    global_ident: Option<(u8, Location)>,
    is_script: bool,
    implicit_return: bool,
) -> Catch {
    // TODO this function could be called called rercursivelly due to the recursive decent nature of the parser, we should add a check to make sure we don't overflow the stack
    devnote!(this "build_function");
    let mut sidelined_func = FunctionObject::new(Some(ident), is_script);
    this.swap_function(&mut sidelined_func);
    begin_scope(this);
    begin_functional_scope(this);
    expect_token!(this OpenParen);
    let mut arity = 0;
    if let Token::Identifier(_) = this.peek()? {
        arity += 1;
        build_param(this)?;

        while let Token::Comma = this.peek()? {
            this.eat();
            arity += 1;
            if arity > 255 {
                // TODO we should use an arity value on the function object but let's make it only exist on compile time
                return Err(this.error_at(SiltError::TooManyParameters));
            }
            build_param(this)?;
        }
    }

    block(this)?;
    if let &OpCode::RETURN = this.read_last_code() {
    } else {
        // TODO if last was semicolon we also push a nil
        if !implicit_return {
            this.emit_at(OpCode::NIL);
        }
        this.emit_at(OpCode::RETURN);
    }

    // TODO why do we need to eat again? This prevents an expression_statement of "End" being called but block should have eaten it?
    // if let Token::End = this.peek()? {
    //     this.eat();
    // }

    end_scope(this, true);
    let upvals = end_functional_scope(this);
    // When we're done compiling the function object we drop the current body function back in and push the compiled func as a constant within that body
    this.swap_function(&mut sidelined_func);
    sidelined_func.upvalue_count = upvals.len() as u8;
    let func_value = Value::Function(Rc::new(sidelined_func));
    if true {
        // need closure
        let constant = this.body.chunk.write_constant(func_value) as u8;
        this.emit_at(OpCode::CLOSURE { constant });
        // emit upvalues

        for val in upvals.iter() {
            // TODO is it worth specifying difference between first function enclosure from higher functional enclosure?
            this.emit_at(OpCode::REGISTER_UPVALUE {
                index: val.ident,
                neighboring: val.neighboring,
            });
        }
    } else {
        // no closure needed
        this.constant_at(func_value);
    }
    define_variable(this, global_ident)?;

    Ok(())
}

fn build_param(this: &mut Compiler) -> Catch {
    let (res, _) = this.pop();
    match res? {
        Token::Identifier(ident) => {
            add_local(this, ident)?;
        }
        _ => {
            return Err(this.error_at(SiltError::ExpectedLocalIdentifier));
        }
    }
    Ok(())
}

fn statement(this: &mut Compiler) -> Catch {
    devnote!(this "statement");
    match this.peek()? {
        Token::Print => print(this)?,
        Token::If => if_statement(this)?,
        Token::Do => {
            this.eat();
            begin_scope(this);
            block(this)?;
            end_scope(this, false);
        }
        Token::While => while_statement(this)?,
        Token::For => for_statement(this)?,
        Token::Return => return_statement(this)?,
        // Token::OpenBrace => block(this),
        Token::ColonColon => set_goto_label(this)?,
        Token::Goto => goto_statement(this)?,
        Token::SemiColon => {
            this.eat();
            // TODO ???
        }
        // Token::End => {
        //     // this.eat();
        //     // TODO ???
        // }
        _ => expression_statement(this)?,
    }
    Ok(())
}

fn block(this: &mut Compiler) -> Catch {
    devnote!(this "block");
    build_block_until!(this, End);

    Ok(())
}

/** lower scope depth */
fn begin_scope(this: &mut Compiler) {
    this.scope_depth += 1;
}

/** Descend into a function's scope and start a new upvalue vec representing required values a level above us */
fn begin_functional_scope(this: &mut Compiler) {
    this.functional_depth += 1;
    this.up_values.push(vec![]);
    this.local_functional_offset.push(this.local_count);
}

/** raise scope depth and dump lowest locals off the imaginary stack */
fn end_scope(this: &mut Compiler, skip_code: bool) {
    this.scope_depth -= 1;

    let mut last_was_pop = true;
    let mut count = 0;
    let mut v = vec![];
    while !this.locals.is_empty() && this.locals.last().unwrap().depth > this.scope_depth {
        let l = this.locals.pop().unwrap();
        if l.is_captured {
            if last_was_pop {
                v.push(count);
                count = 0;
            }
            last_was_pop = false;
            count += 1;
        } else {
            if !last_was_pop {
                v.push(count);
                count = 0;
            }
            last_was_pop = true;
            count += 1;
        }
    }
    if count > 0 {
        v.push(count);
    }
    // if we're not dealing with upvalues and we're skipping code due to functional scope our stack will get moved off anyway
    if skip_code {
        //&& v.len() <= 1 {
        return;
    }

    // index 0 is always OP_POPS but could be count of 0 if the first local is captured. Otherwise we can safely stagger even as pop, odds as close
    v.iter().enumerate().for_each(|(i, c)| {
        if i % 2 == 0 {
            this.emit_at(OpCode::POPS(*c));
        } else {
            this.emit_at(OpCode::CLOSE_UPVALUES(*c));
        }
    });
}

/** raise functional depth */
fn end_functional_scope(this: &mut Compiler) -> Vec<UpLocal> {
    this.functional_depth -= 1;
    this.local_functional_offset.pop();
    this.up_values.pop().unwrap()
}

fn if_statement(this: &mut Compiler) -> Catch {
    devnote!(this "if_statement");
    this.eat();
    expression(this, false)?;
    expect_token!(this Then);
    let index = this.emit_index(OpCode::GOTO_IF_FALSE(0));
    this.emit_at(OpCode::POP);
    build_block_until!(this, End | Else | ElseIf);
    // let else_jump = this.emit_index(OpCode::FORWARD(0));
    this.patch(index)?;
    this.emit_at(OpCode::POP);
    match this.peek()? {
        Token::Else => {
            this.eat();
            let index = this.emit_index(OpCode::FORWARD(0));
            build_block_until!(this, End);
            this.patch(index)?;
        }
        Token::ElseIf => {
            this.eat();
            this.emit_at(OpCode::POP);
            if_statement(this)?;
        }
        _ => {}
    }
    Ok(())
}

fn while_statement(this: &mut Compiler) -> Catch {
    devnote!(this "while_statement");
    this.eat();
    let loop_start = this.get_chunk_size();
    expression(this, false)?;
    expect_token!(this Do);
    let exit_jump = this.emit_index(OpCode::GOTO_IF_FALSE(0));
    this.emit_at(OpCode::POP);
    build_block_until!(this, End);
    this.emit_rewind(loop_start);
    this.patch(exit_jump)?;
    this.emit_at(OpCode::POP);
    Ok(())
}

fn for_statement(this: &mut Compiler) -> Catch {
    devnote!(this "for_statement");
    this.eat();
    let pair = this.pop();
    let t = pair.0?;
    if let Token::Identifier(ident) = t {
        begin_scope(this);
        let iterator = add_local(this, ident)?; // reserve iterator by identifier
        expect_token!(this Assign);
        let comparison = add_local_placeholder(this)?; // reserve end value with placeholder
        let step_value = add_local_placeholder(this)?; // reserve step value with placeholder
        expression(this, false)?; // expression for iterator
        expect_token!(this Comma);
        expression(this, false)?; // expression for end value

        // let exit_jump = this.emit_index(OpCode::GOTO_IF_FALSE(0));
        // this.emit_at(OpCode::POP);
        // either we have an expression for the step or we set it to 1i
        if let Token::Comma = this.peek()? {
            this.eat();
            expression(this, false)?;
        } else {
            this.constant_at(Value::Integer(1))
        };
        this.emit_at(OpCode::GET_LOCAL { index: iterator });
        let loop_start = this.get_chunk_size();
        // compare iterator to end value
        this.emit_at(OpCode::GET_LOCAL { index: comparison });
        this.emit_at(OpCode::EQUAL);
        let exit_jump = this.emit_index(OpCode::GOTO_IF_TRUE(0));
        this.emit_at(OpCode::POP);
        expect_token!(this Do);

        // this.emit_at(OpCode::POP);
        // statement(this)?;
        build_block_until!(this, End);
        this.emit_at(OpCode::GET_LOCAL { index: iterator });
        this.emit_at(OpCode::GET_LOCAL { index: step_value });
        this.emit_at(OpCode::ADD);
        this.emit_at(OpCode::SET_LOCAL { index: iterator });
        this.emit_rewind(loop_start);
        this.patch(exit_jump)?;
        this.emit_at(OpCode::POP);
        end_scope(this, false);
        Ok(())
    } else {
        Err(this.error_at(SiltError::ExpectedLocalIdentifier))
    }
}
fn return_statement(this: &mut Compiler) -> Catch {
    devnote!(this "return_statement");
    this.eat();
    if let Token::End | Token::EOF = this.peek()? {
        this.emit_at(OpCode::NIL);
    } else {
        expression(this, false)?;
    }
    this.emit_at(OpCode::RETURN);
    Ok(())
}

fn set_goto_label(this: &mut Compiler) -> Catch {
    devnote!(this "goto_label");
    this.eat();
    let token = this.pop().0?;
    if let Token::Identifier(ident) = token {
        this.labels.insert(*ident, this.get_chunk_size());
    } else {
        return Err(this.error_at(SiltError::ExpectedLabelIdentifier));
    }
    Ok(())
}

fn goto_statement(this: &mut Compiler) -> Catch {
    devnote!(this "goto_statement");
    this.eat();
    let token = this.pop().0?;
    if let Token::Identifier(ident) = token {
        resolve_goto(this, &*ident, this.get_chunk_size(), None)?;
        // match this.labels.get(ident) {
        //     Some(i) => {
        //         let c = this.chunk.code.len();
        //         let o = *i;
        //         if c > o {
        //             let offset = c - o;
        //             if offset > u16::MAX as usize {
        //                 return Err(this.error_at(SiltError::TooManyOperations));
        //             }
        //             this.emit_at(OpCode::REWIND(offset as u16));
        //         } else {
        //             let offset = o - c;
        //             if offset > u16::MAX as usize {
        //                 return Err(this.error_at(SiltError::TooManyOperations));
        //             }
        //             this.emit_at(OpCode::FORWARD(offset as u16));
        //         }
        //     }
        //     None => {
        //         let index = this.emit_index(OpCode::FORWARD(0));
        //         this.pending_gotos.push((ident, index));
        //         // this.labels.insert(*ident, index);
        //     }
        // }
    } else {
        return Err(this.error_at(SiltError::ExpectedGotoIdentifier));
    }

    // n end_scope(this: &mut Compiler) {
    //     this.scope_depth -= 1;
    //     let mut i = 0;
    //     while !this.locals.is_empty() && this.locals.last().unwrap().depth > this.scope_depth {
    //         this.locals.pop();
    //         i += 1;
    //     }
    //     this.emit_at(OpCode::POPN(i));
    // }

    Ok(())
}

fn resolve_goto(
    this: &mut Compiler,
    ident: &str,
    op_count: usize,
    replace: Option<(usize, Location)>,
) -> Catch {
    match this.labels.get(ident) {
        Some(i) => {
            let c = op_count;
            let o = *i;
            let code = if c > o {
                let offset = c - o;
                if offset > u16::MAX as usize {
                    return Err(this.error_at(SiltError::TooManyOperations));
                }
                OpCode::REWIND(offset as u16)
            } else {
                let offset = o - c;
                if offset > u16::MAX as usize {
                    return Err(this.error_at(SiltError::TooManyOperations));
                }
                OpCode::FORWARD(offset as u16)
            };

            match replace {
                Some((i, _)) => {
                    this.get_chunk_mut().code[i] = code;
                }
                None => {
                    this.emit_at(code);
                }
            }
        }
        None => match replace {
            Some((i, location)) => {
                return Err(this.error_syntax(SiltError::UndefinedLabel(ident.to_owned()), location))
            }
            None => {
                let index = this.emit_index(OpCode::FORWARD(0));
                this.pending_gotos
                    .push((ident.to_owned(), index, this.current_location));
            } // this.labels.insert(*ident, index);
        },
    };
    Ok(())
}

fn final_resolve_goto(this: &mut Compiler) {
    this.pending_gotos
        .iter()
        .for_each(|(ident, index, location)| {});
}

fn goto_scope_skip(this: &mut Compiler) {
    if this.locals.is_empty() {
        return;
    }
    let mut i = 0;
    this.locals
        .iter()
        .rev()
        .take_while(|l| l.depth > this.scope_depth)
        .for_each(|_| {
            i += 1;
        });

    this.emit_at(OpCode::POPS(i));
}

fn expression(this: &mut Compiler, skip_step: bool) -> Catch {
    devnote!(this "expression");
    this.parse_precedence(Precedence::Assignment, skip_step)?;
    Ok(())
}

fn next_expression(this: &mut Compiler) -> Catch {
    devnote!(this "next_expression");
    this.eat();
    expression(this, false)?;
    Ok(())
}

fn expression_statement(this: &mut Compiler) -> Catch {
    devnote!(this "expression_statement");
    expression(this, false)?;
    if this.override_pop {
        this.override_pop = false;
    } else {
        this.emit_at(OpCode::POP);
    }
    devnote!(this "expression_statement end");
    Ok(())
}

fn variable(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "variable");
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

fn named_variable(this: &mut Compiler, can_assign: bool, mut local: bool) -> Catch {
    devnote!(this "named_variables");
    let t = this.copy_store()?;

    // declare shortcut only valid if we're above scope 0 otherwise it's redundant
    // TODO should we send a warning that 0 scoped declares are redundant?
    if this.scope_depth > 0
        && if let Token::Op(Operator::ColonEquals) = this.peek()? {
            true
        } else {
            false
        }
    {
        if let Token::Identifier(ident) = t {
            // short declare
            add_local(this, ident)?;
            this.override_pop = true;
            this.eat();
            expression(this, false)?;
        } else {
            unreachable!()
        }
        return Ok(());
    }

    //normal assigment
    let (setter, getter) = if let Token::Identifier(ident) = t {
        devout!("assigning to identifier: {}", ident);
        // TODO currently this mechanism searches the entire local stack to determine local and then up values,  ideally we check up values first once we raise out of the functional scope instead of continuing to walk the local stack, but this will work for now.
        match resolve_local(this, &ident) {
            Some((i, is_up)) => {
                if is_up {
                    (
                        OpCode::SET_UPVALUE { index: i },
                        OpCode::GET_UPVALUE { index: i },
                    )
                } else {
                    (
                        OpCode::SET_LOCAL { index: i },
                        OpCode::GET_LOCAL { index: i },
                    )
                }
            }
            None => {
                let ident = this.identifer_constant(ident);
                // add_upvalue(this, ident, this.scope_depth);
                (
                    OpCode::SET_GLOBAL { constant: ident },
                    OpCode::GET_GLOBAL { constant: ident },
                )
            }
        }
    } else {
        unreachable!()
    };

    /*
     * normally:
     * expression for value
     * set_var id
     *
     * with table:
     * push table ref onto stack
     * expression for index OR constant for index
     * repeat for each chained index and remember count
     * expression for value
     * set_table with depth = index count
     *
     *
     */
    match this.peek()? {
        Token::Assign => {
            if can_assign {
                this.eat();
                expression(this, false)?;
                this.emit_at(setter);
            } else {
                this.emit_at(getter);
            }
        }
        Token::OpenBracket | Token::Dot => {
            this.emit_at(getter);
            let count = table_indexer(this)? as u8;
            if let Token::Assign = this.peek()? {
                this.eat();
                expression(this, false)?;
                this.emit_at(OpCode::TABLE_SET { depth: count });
                // override statement end pop because instruction takes care of it
                this.override_pop = true;
            } else {
                this.emit_at(OpCode::TABLE_GET { depth: count });
            }
        }
        _ => this.emit_at(getter),
    }

    // if can_assign
    //     && if let Token::Assign = this.peek()? {
    //         true
    //     } else {
    //         false
    //     }
    // {
    //     this.eat();
    //     expression(this, false)?;
    //     this.emit_at(setter);
    // } else {
    //     this.emit_at(getter);
    // }

    // if let &Token::Assign = this.get_current()? {
    //     let loc = this.current_location;
    //     expression(this, false)?;
    //     this.emit(OpCode::DEFINE_GLOBAL { constant: ident }, loc);
    // } else {
    //     this.emit_at(OpCode::GET_GLOBAL { constant: ident });
    // }
    Ok(())
}

fn grouping(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "grouping");
    expression(this, false)?;
    //TODO expect
    // expect_token!(self, CloseParen, SiltError::UnterminatedParenthesis(0, 0));
    // self.consume(TokenType::RightParen, "Expect ')' after expression.");
    Ok(())
}

fn tabulate(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "tabulate");
    this.emit_at(OpCode::NEW_TABLE);
    // not immediately closed
    if !matches!(this.peek()?, &Token::CloseBrace) {
        let mut count = 0;
        // check if index provided via brackets or ident, otherwise increment our count to build array at the end
        while {
            let start_brace = this.current_location;
            if match this.peek()? {
                Token::Identifier(_) => {
                    this.store();
                    if let Token::Assign = this.peek()? {
                        let ident = this.get_current()?.unwrap_identifier();
                        this.emit_identifer_constant_at(ident.clone());
                        this.eat();
                        true
                    } else {
                        expression(this, true)?; // we skip the store because the ip is already where it needs to be
                        false
                    }
                }
                Token::OpenBracket => {
                    this.eat();
                    expression(this, false)?;
                    expect_token!(
                        this,
                        CloseBracket,
                        this.error_at(SiltError::UnterminatedBracket(start_brace.0, start_brace.1))
                    );
                    expect_token!(this, Assign, this.error_at(SiltError::ExpectedAssign));
                    true
                }
                _ => {
                    expression(this, false)?; // normal store expression
                    false
                }
            } {
                expression(this, false)?;
                this.emit_at(OpCode::TABLE_INSERT { offset: count });
            } else {
                count += 1;
            }

            match this.peek()? {
                Token::Comma => {
                    this.eat();
                    true
                }
                Token::CloseBrace => false,
                _ => return Err(this.error_at(SiltError::TableExpectedCommaOrCloseBrace)),
            }
        } {
            // if args >= 255 {
            //     return Err(this.error_at(SiltError::TooManyParameters));
            // }
        }
        if count > 0 {
            this.emit_at(OpCode::TABLE_BUILD(count));
        }
    }

    expect_token!(
        this,
        CloseBrace,
        this.error_at(SiltError::TableExpectedCommaOrCloseBrace)
    );
    Ok(())
}

/** op unary or primary */
fn unary(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "unary");
    let t = this.copy_store()?;
    // self.expression();

    this.parse_precedence(Precedence::Unary, false)?;
    match t {
        Token::Op(Operator::Sub) => this.emit_at(OpCode::NEGATE),
        Token::Op(Operator::Not) => this.emit_at(OpCode::NOT),
        Token::Op(Operator::Length) => this.emit_at(OpCode::LENGTH),
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

fn table_indexer(this: &mut Compiler) -> Result<usize, ErrorTuple> {
    let mut count = 0;
    while match this.peek()? {
        Token::OpenBracket => {
            this.eat();
            expression(this, false)?;
            expect_token!(
                this,
                CloseBracket,
                this.error_at(SiltError::UnterminatedBracket(0, 0))
            );
            true
        }
        Token::Dot => {
            this.eat();
            let t = this.pop();
            this.current_location = t.1;
            let field = t.0?;
            devout!("table_indexer: {} p:{}", field, this.peek()?);
            if let Token::Identifier(ident) = field {
                this.emit_identifer_constant_at(ident);
            } else {
                return Err(this.error_at(SiltError::ExpectedFieldIdentifier));
            }
            true
        }
        _ => false,
    } {
        count += 1;
    }
    Ok(count)
}

fn binary(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "binary");
    let t = this.copy_store()?;
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
    devnote!(this "concat_binary");
    let t = this.copy_store()?;
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

fn and(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "and");
    let index = this.emit_index(OpCode::GOTO_IF_FALSE(0));
    this.emit_at(OpCode::POP);
    this.parse_precedence(Precedence::And, false)?;
    this.patch(index)?;
    Ok(())
}

fn or(this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(this "or");

    // the goofy way
    // let index = this.emit_index(OpCode::GOTO_IF_FALSE(0));
    // let final_index = this.emit_index(OpCode::GOTO(0));
    // this.patch(index)?;
    // this.emit_at(OpCode::POP);
    // this.parse_precedence(Precedence::Or, false)?;
    // this.patch(final_index)?;

    let index = this.emit_index(OpCode::GOTO_IF_TRUE(0));
    this.emit_at(OpCode::POP);
    this.parse_precedence(Precedence::Or, false)?;
    this.patch(index)?;
    Ok(())
}

fn integer(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "integer");
    let t = this.copy_store()?;
    let value = if let Token::Integer(i) = t {
        Value::Integer(i)
    } else {
        unreachable!()
    };
    this.constant_at(value);
    Ok(())
}

fn number(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "number");
    let t = this.copy_store()?;
    let value = if let Token::Number(n) = t {
        Value::Number(n)
    } else {
        unreachable!()
    };
    this.constant_at(value);
    Ok(())
}

fn string(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "string");
    let t = this.copy_store()?;
    let value = if let Token::StringLiteral(s) = t {
        Value::String(Box::new(s.into_string()))
    } else {
        unreachable!()
    };
    this.constant_at(value);
    Ok(())
}

fn literal(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "literal");
    let t = this.copy_store()?;
    match t {
        Token::Nil => this.emit_at(OpCode::NIL),
        Token::True => this.emit_at(OpCode::TRUE),
        Token::False => this.emit_at(OpCode::FALSE),
        _ => unreachable!(),
    }
    Ok(())
}

fn call(this: &mut Compiler, can_assign: bool) -> Catch {
    devnote!(this "call");
    // let t = this.take_store()?;
    // let l = this.current_location;
    // let rule = Compiler::get_rule(&t);
    // this.parse_precedence(rule.precedence.next(), false)?;
    // if let Token::OpenParen = t {
    //     let arg_count = this.argument_list()?;
    //     this.emit(OpCode::CALL { arg_count }, l);
    // }
    let start = this.current_location;
    let arg_count = arguments(this, start)?;
    this.emit(OpCode::CALL(arg_count), start);
    Ok(())
}

fn call_table(this: &mut Compiler, can_assign: bool) -> Catch {
    todo!();
    Ok(())
}

fn call_string(this: &mut Compiler, can_assign: bool) -> Catch {
    todo!();
    Ok(())
}
fn arguments(this: &mut Compiler, start: Location) -> Result<u8, ErrorTuple> {
    devnote!(this "arguments");
    let mut args = 0;
    if !matches!(this.peek()?, &Token::CloseParen) {
        while {
            expression(this, false)?;
            args += 1;
            if let &Token::Comma = this.peek()? {
                this.eat();
                true
            } else {
                false
            }
        } {
            if args >= 255 {
                return Err(this.error_at(SiltError::TooManyParameters));
            }
        }
    }

    expect_token!(
        this,
        CloseParen,
        this.error_at(SiltError::UnterminatedParenthesis(start.0, start.1))
    );
    devout!("arguments count: {}", args);

    Ok(args)
}

fn print(this: &mut Compiler) -> Catch {
    devnote!(this "print");
    this.eat();
    expression(this, false)?;
    this.emit_at(OpCode::PRINT);
    Ok(())
}

pub fn void(_this: &mut Compiler, _can_assign: bool) -> Catch {
    devnote!(_this "void");
    Ok(())
}

// pub fn invalid(_: &mut Compiler) { // TODO
//                                    // this.error(SiltError::InvalidExpression);
// }

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
