use std::{
    cmp::Ordering,
    fmt::{Display, Formatter},
    iter::Peekable,
    println, vec,
};

use gc_arena::{Gc, Mutation};
use hashbrown::HashMap;

use crate::{
    code::OpCode,
    error::{ErrorTuple, SiltError, TokenCell},
    function::FunctionObject,
    lexer::{Lexer, TokenTripleResult},
    token::{Operator, Token},
    value::Value,
};

use colored::Colorize;

macro_rules! build_block_until_then_eat {
    ($self:ident, $mc:ident, $f:ident, $it:ident, $($rule:ident)|*) => {{
        while match $self.peek($it)? {
            $( Token::$rule)|* => {$self.eat($it); false}
            Token::EOF => {
                return Err($self.error_at(SiltError::UnterminatedBlock));
            }
            _ =>{declaration($self, $mc, $f, $it)?; true}
        } {
        }
    }};
}

macro_rules! build_block_until {
    ($self:ident, $mc:ident, $f:ident, $it:ident, $($rule:ident)|*) => {{
        while match $self.peek($it)? {
            $( Token::$rule)|* => false,
            Token::EOF => {
                return Err($self.error_at(SiltError::UnterminatedBlock));
            }
            _ =>{declaration($self, $mc, $f, $it)?; true}
        } {
        }
    }};
}

macro_rules! scope_and_block_until {
    ($self:ident, $mc:ident, $f:ident, $it:ident, $($rule:ident)|*) => {{
        begin_scope($self);
        build_block_until!($self, $mc, $f, $it, $($rule)|*);
        end_scope($self, $f, false);
    }};
}

// macro_rules! scope_and_block_until_then_eat {
//     ($self:ident,$e:ident,, $($rule:ident)|*) => {{
//         begin_scope($self);
//         build_block_until_then_eat!($self,$e:ident, $($rule)|*);
//         end_scope($self,false);
//     }};
// }

macro_rules! devnote {
    ($self:ident $it:ident $message:literal) => {
        #[cfg(feature = "dev-out")]
        println!(
            "=> {}: peek: {:?} -> current: {:?}",
            $message,
            $self.peek($it).unwrap_or(&Token::Nil).clone(),
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

// macro_rules! op_assign {
//     ($self:ident, $ident:ident,$op:ident) => {{
//         let value = $self.expression();
//         let bin = Expression::Binary {
//             left: Box::new(Expression::Variable {
//                 $ident,
//                 location: $self.get_last_loc(),
//             }),
//             operator: Operator::$op,
//             right: Box::new(value),
//             location: $self.get_last_loc(),
//         };
//         Expression::Assign {
//             ident: $ident,
//             value: Box::new(bin),
//             location: $self.get_last_loc(),
//         }
//     }};
// }

/** error if missing, eat if present */
macro_rules! expect_token {
    ($self:ident $it:ident $token:ident) => {{
        if let Token::$token = $self.peek($it)? {
            $self.eat($it);
        } else {
            return Err($self.error_at(SiltError::ExpectedToken(Token::$token)));
        }
    };};
    ($self:ident, $it:ident, $token:ident, $custom_error:expr) => {{
        if let Token::$token = $self.peek($it)? {
            $self.eat($it);
        } else {
            return Err($custom_error);
        }
    };};
}

macro_rules! add {
    ($self:ident) => {{
        $self.expression_count += 1;
        #[cfg(feature = "dev-out")]
        println!("{} {}", "Add".on_cyan(), $self.expression_count);
    };};
}

// macro_rules! expect_token_exp {
//     ($self:ident $token:ident) => {{
//         if let Some(&Token::$token) = $self.peek() {
//             $self.eat();
//         } else {
//             $self.error(SiltError::ExpectedToken(Token::$token));
//             return Expression::InvalidExpression;
//         }
//     };};
// }

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

#[derive(Clone, Copy, Debug)]
pub struct LanguageFlags {
    pub implicit_returns: bool,
    pub arrow_functions: bool,
    pub bang_operator: bool,
}

impl Default for LanguageFlags {
    fn default() -> Self {
        Self {
            implicit_returns: false,
            arrow_functions: false,
            bang_operator: false,
        }
    }
}

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
    prefix: fn(&mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, can_assign: bool) -> Catch,
    infix: fn(&mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, can_assign: bool) -> Catch,
    precedence: Precedence,
}

/** stores a local identifier's name by boxed string, if none is provbided it serves as a placeholder for statements such as a loop, this way they cannot be resolved as variables */
struct Local {
    ident: Option<String>,
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

type FnRef<'a, 'c> = &'a mut FunctionObject<'c>;

pub struct Compiler {
    // pub body: FunctionObject<'chnk>,
    pub current_index: usize,
    pub errors: Vec<ErrorTuple>,
    pub valid: bool,
    current: Result<Token, ErrorTuple>,
    current_location: TokenCell,
    scope_depth: usize,
    functional_depth: usize,
    // TODO we need a fail catch if we exceed a local variable amount of up values as well
    up_values: Vec<Vec<UpLocal>>,
    local_offset: Vec<usize>,
    /** an offset tracker each time we descend into a new functional scope. For instance if we drop 1 level down from the root level that had 3 locals prior [A,B,C] then our stack looks like [root, A, B, C, fn] then we'll store 3 at this field's index 0 since the calling function is always at the bottom of the stack */
    local_functional_offset: Vec<usize>,
    locals: Vec<Local>,
    local_count: usize,
    labels: HashMap<String, usize>,
    // location: (usize, usize),
    // previous: TokenTuple,
    // pre_previous: TokenTuple,
    pending_gotos: Vec<(String, usize, TokenCell)>,
    /** hack to flip off a pop when an expression takes on statement properties, used only for := right now */
    override_pop: bool,
    /** language flags for optional features */
    language_flags: LanguageFlags,
    /** tracks if the last statement was an expression for implicit returns */
    last_was_expression: bool,
    // return_count: u8,
    /** tracks the number of values on the stack from comma-separated expressions */
    expression_count: u8,
    /// used for multi var assignment, start small, expand if really necessary
    var_stack: Vec<(OpCode, OpCode)>,
    /// men will do anything to not have to allocate a new vec
    var_set_stack: Vec<(OpCode, OpCode)>,
}

impl Compiler {
    /** Create a new compiler instance */
    pub fn new() -> Compiler {
        // assert!(p.len() == p.len());
        Self {
            // body: FunctionObject::new(None, true),
            current: Ok(Token::Nil),
            current_location: (0, 0),
            current_index: 0,
            errors: vec![],
            valid: true,
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
            local_offset: vec![],
            local_count: 1,
            labels: HashMap::new(),
            pending_gotos: vec![],
            // location: (0, 0),
            // previous: (Token::Nil, (0, 0)),
            // pre_previous: (Token::Nil, (0, 0)),
            override_pop: false,
            language_flags: LanguageFlags::default(),
            last_was_expression: false,
            expression_count: 0,
            var_stack: Vec::with_capacity(4),
            var_set_stack: Vec::with_capacity(4),
        }
    }

    /** Create a new compiler instance with language flags */
    pub fn new_with_flags(
        implicit_returns: bool,
        arrow_functions: bool,
        bang_operator: bool,
    ) -> Compiler {
        let mut compiler = Self::new();
        compiler.language_flags = LanguageFlags {
            implicit_returns,
            arrow_functions,
            bang_operator,
        };
        compiler
    }

    /** Syntax error with code at location */
    fn error_syntax(&mut self, code: SiltError, location: TokenCell) -> ErrorTuple {
        self.valid = false;
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

    // fn get_chunk(&self,e: &'a mut Emphereal) -> &'chnk Chunk {
    //     &e.body.chunk
    // }

    // fn get_chunk_mut(&mut self) -> &'chnk mut Chunk {
    //     &mut self.body.chunk
    // }

    fn get_chunk_size(&self, f: FnRef) -> usize {
        f.chunk.code.len()
    }

    fn write_code(&self, f: FnRef, byte: OpCode, location: TokenCell) -> usize {
        f.chunk.write_code(byte, location)
    }

    // fn read_last_code<'a, 'c: 'a>(&self, e: &mut Emphereal<'a,'c>) -> &'c OpCode {
    //     e.body.chunk.code.last().unwrap()
    //     // self.code.last().unwrap()
    // }

    fn write_identifier(&mut self, f: FnRef, identifier: String) -> usize {
        f.chunk.write_identifier(identifier)
    }

    fn change_code(&mut self, f: FnRef, offset: usize, byte: OpCode) {
        f.chunk.code[offset] = byte;
    }
    fn get_code<'a, 'c: 'a>(&self, f: FnRef<'a, 'c>, offset: usize) -> &'a OpCode {
        &f.chunk.code[offset]
    }

    /** Tokens, not stack. Pop and return the token tuple, take care as this does not wipe the current token but does advance the iterator */
    fn pop(&mut self, iter: &mut Peekable<Lexer>) -> (Result<Token, ErrorTuple>, TokenCell) {
        self.current_index += 1;
        match iter.next() {
            Some(Ok(t)) => {
                // devout!("popped {}", t.0);
                (Ok(t.0), (t.1 .0, t.1 .2))
            }
            Some(Err(e)) => {
                let l = e.location;
                (Err(e), l)
            }
            None => (Ok(Token::EOF), (1, 0)),
        }
    }

    /** Force stack to pop N values without usual niceties, this both emits opcode and drops off the emulated stack locals */
    fn force_stack_pop(&mut self, f: FnRef, n: usize) {
        self.locals.truncate(self.locals.len() - n);
        self.emit_at(f, OpCode::POPS(n as u8));
    }

    /** Slightly faster pop that devourse the token or error, should follow a peek or risk skipping as possible error. Probably irrelevant otherwise. */
    fn eat(&mut self, iter: &mut Peekable<Lexer>) {
        self.current_index += 1;
        let t = iter.next();
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
    fn store(&mut self, iter: &mut Peekable<Lexer>) {
        self.current_index += 1;
        (self.current, self.current_location) = match iter.next() {
            Some(Ok(t)) => (Ok(t.0), (t.1 .0, t.1 .2)),
            Some(Err(er)) => {
                // self.error_syntax(e.code, e.location);
                let l = er.location;
                (Err(er), l)
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
    fn store_and_return(&mut self, iter: &mut Peekable<Lexer>) -> Result<Token, ErrorTuple> {
        self.current_index += 1;
        let (r, l) = match iter.next() {
            Some(Ok(t)) => (Ok(t.0), (t.1 .0, t.1 .2)),
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
                // devout!("get_current {}", t);
                Ok(t)
            }
            Err(e) => Err(e.clone()),
        }
    }

    fn drain_setters(&mut self, f: FnRef) {
        let vv = self.var_set_stack.drain(..).rev();
        let mut it = vv.peekable();
        while let Some(v) = it.next() {
            f.chunk.write_code(v.0, self.current_location);
            f.chunk.write_code(OpCode::POP, self.current_location);
        }
    }

    fn drain_getters(&mut self, f: FnRef) {
        for v in self.var_stack.drain(..) {
            f.chunk.write_code(v.1, self.current_location);
        }
    }

    /** only use after peek */
    // pub fn eat_out(&mut self) -> TokenResult {
    //     self.current_index += 1;
    //     self.iterator.next().unwrap()
    // }

    /** return the peeked token result */
    fn peek<'c>(&mut self, iter: &'c mut Peekable<Lexer>) -> Result<&'c Token, ErrorTuple> {
        match iter.peek() {
            Some(Ok(t)) => {
                // devout!("peek {}", t.0);
                Ok(&t.0)
            }
            Some(Err(e)) => {
                // self.error_syntax(e.code, e.location);
                devout!("!! peek err {}", e.code);
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
    fn peek_result<'c>(&mut self, iter: &'c mut Peekable<Lexer>) -> &'c TokenTripleResult {
        #[cfg(feature = "dev-out")]
        match iter.peek() {
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
            None => &Ok((Token::EOF, (0, 0, 0))),
        }
        #[cfg(not(feature = "dev-out"))]
        match iter.peek() {
            Some(r) => r,
            None => &Ok((Token::EOF, (0, 0, 0))),
        }
    }

    /** emit op code at token location */
    fn emit<'a, 'c: 'a>(&mut self, f: FnRef, op: OpCode, location: TokenCell) {
        #[cfg(feature = "dev-out")]
        {
            println!("emit ... {}", op);
            // self.chunk.print_chunk();
        }
        self.write_code(f, op, location);
    }

    /** emit op code at current token location */
    fn emit_at(&self, f: FnRef, op: OpCode) {
        #[cfg(feature = "dev-out")]
        {
            println!("emit_at ...{}", op);
            // self.chunk.print_chunk()
        }
        self.write_code(f, op, self.current_location);
    }

    /** emit op code at current token location */
    fn drop_last_if(&mut self, f: FnRef, op: &OpCode) {
        let b = f.chunk.drop_last_if(op);
        #[cfg(feature = "dev-out")]
        {
            println!("{} drop_last_if {}? {}!", "DROP".on_red(), op, b);
            // self.chunk.print_chunk()
        }
    }

    /** emit op code at current token location and return op index */
    fn emit_index(&mut self, f: FnRef, op: OpCode) -> usize {
        #[cfg(feature = "dev-out")]
        {
            println!("emit_index ... {}", op);
            // frame.chunk.print_chunk()
        }
        self.write_code(f, op, self.current_location)
    }

    /** patch the op code that specified index */
    fn patch(&mut self, f: FnRef, offset: usize) -> Catch {
        let jump = self.get_chunk_size(f) - offset - 1;
        if jump > u16::MAX as usize {
            return Err(self.error_at(SiltError::TooManyOperations));
        }
        // self.chunk.code[offset] = ((jump >> 8) & 0xff) as u8;
        // self.chunk.code[offset + 1] = (jump & 0xff) as u8;
        let c = self.get_code(f, offset);
        match c {
            OpCode::GOTO_IF_FALSE(_) => {
                self.change_code(f, offset, OpCode::GOTO_IF_FALSE(jump as u16));
            }
            OpCode::GOTO_IF_TRUE(_) => {
                self.change_code(f, offset, OpCode::GOTO_IF_TRUE(jump as u16));
            }
            OpCode::POP_AND_GOTO_IF_FALSE(_) => {
                self.change_code(f, offset, OpCode::POP_AND_GOTO_IF_FALSE(jump as u16));
            }
            OpCode::FORWARD(_) => self.change_code(f, offset, OpCode::FORWARD(jump as u16)),
            OpCode::REWIND(_) => self.change_code(f, offset, OpCode::REWIND(jump as u16)),
            OpCode::FOR_NUMERIC(_) => self.change_code(f, offset, OpCode::FOR_NUMERIC(jump as u16)),
            _ => {
                return Err(self.error_at(SiltError::ChunkCorrupt));
            }
        }
        Ok(())
    }

    fn emit_rewind(&mut self, f: FnRef, start: usize) {
        // we base the jump off of the index we'll be at one we've written the rewind op below
        let jump = (self.get_chunk_size(f) + 1) - start;
        if jump > u16::MAX as usize {
            self.error_at(SiltError::TooManyOperations);
        }
        self.write_code(f, OpCode::REWIND(jump as u16), self.current_location);
    }

    fn set_label(&mut self, f: FnRef, label: String) {
        self.labels.insert(label, self.get_chunk_size(f));
    }

    fn identifer_constant(&mut self, f: FnRef, ident: String) -> u8 {
        self.write_identifier(f, ident) as u8
    }

    /** write to constant table  */
    fn write_constant<'a, 'c: 'a>(&mut self, f: FnRef<'a, 'c>, value: Value<'c>) -> u8 {
        f.chunk.write_constant(value) as u8
    }

    /** write to constant table and emit the op code at location */
    fn constant<'a, 'c: 'a>(&mut self, f: FnRef<'_, 'c>, value: Value<'c>, location: TokenCell) {
        let constant = self.write_constant(f, value);
        self.emit(f, OpCode::CONSTANT { constant }, location);
    }

    /** write to constant table and emit the op code at the current location */
    fn constant_at<'a, 'c: 'a>(&mut self, f: FnRef<'_, 'c>, value: Value<'c>) {
        self.constant(f, value, self.current_location);
    }

    /** write identifier to constant table, remove duplicates, and emit code */
    fn emit_identifer_constant_at<'a, 'c: 'a>(&mut self, f: FnRef, ident: String) {
        let constant = self.write_identifier(f, ident) as u8;
        self.emit(f, OpCode::CONSTANT { constant }, self.current_location);
    }

    fn is_end(&mut self, iter: &mut Peekable<Lexer>) -> bool {
        match iter.peek() {
            None => true,
            _ => false,
        }
    }

    /** replaces the conents of func with the compilers body */
    // fn swap_function(&mut self, func: &mut FunctionObject<'chnk>) {
    //     swap(&mut self.body, func);
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

    fn compile<'c>(
        &mut self,
        mc: &Mutation<'c>,
        name: Option<String>,
        source: &str,
    ) -> FunctionObject<'c> {
        #[cfg(feature = "dev-out")]
        {
            let lexer = Lexer::new(source);
            lexer.for_each(|r| match r {
                Ok(t) => {
                    println!("token {}", t.0);
                }
                Err(e) => println!("err {}", e),
            });
        }
        let lexer = Lexer::new(source);
        let mut body = FunctionObject::new(name, true);
        let mut iter = lexer.peekable();

        while iter.peek().is_some() {
            match declaration(self, mc, &mut body, &mut iter) {
                Ok(()) => {}
                Err(e) => {
                    self.push_error(e);
                    self.synchronize();
                }
            }
        }

        // Handle implicit returns for multiple expressions
        if self.language_flags.implicit_returns && self.last_was_expression {
            // If we have multiple expressions, keep them all on the stack
            // Otherwise, drop the last POP to keep the single expression
            if self.expression_count <= 1 {
                self.drop_last_if(&mut body, &OpCode::POP);
            }
        } else {
            self.drop_last_if(&mut body, &OpCode::POP);
        }
        // self.expression_count we should convert to tuple right? piping to CLI ???
        self.emit(&mut body, OpCode::RETURN(0), (0, 0));
        if !self.valid {
            body.chunk.invalidate();
        }
        body
    }

    pub fn try_compile<'c>(
        &mut self,
        mc: &Mutation<'c>,
        name: Option<String>,
        source: &str,
    ) -> Result<FunctionObject<'c>, Vec<ErrorTuple>> {
        let obj = self.compile(mc, name, source);
        if obj.chunk.is_valid() {
            Ok(obj)
        } else {
            Err(self.pop_errors())
        }
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

    fn parse_precedence(
        &mut self,
        f: FnRef,
        it: &mut Peekable<Lexer>,
        precedence: Precedence,
        skip_step: bool,
    ) -> Catch {
        if !skip_step {
            self.store(it);
        }
        // self.store(); // MARK with store first it works for normal statements, but it breaks for incomplete expressions that are meant to pop off
        // Basically the integer we just saw is dropped off when we reach here because of store
        let t = self.get_current()?;

        devout!("check rule for token {}", t);
        let rule = Self::get_rule(t);
        // #[cfg(feature = "dev-out")]
        devout!(
            "target precedence:  {}, current precedence: {}",
            precedence,
            rule.precedence,
        );
        // if (rule.prefix) != Self::void { // TODO bubble error up if no prefix, call invalid func to bubble?
        let can_assign = precedence <= Precedence::Assignment;
        (rule.prefix)(self, f, it, can_assign)?;

        loop {
            let c = self.peek_result(it);
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
            self.store(it);
            (rule.infix)(self, f, it, false)?;
        }

        // TODO test this with `local b="b" sprint b`
        if can_assign
            && if let Token::Assign = self.peek(it)? {
                true
            } else {
                false
            }
        {
            let res = self.peek(it)?.clone();
            return Err(self.error_at(SiltError::InvalidAssignment(res)));
        }

        // if skip_step {
        //     self.store();
        // }
        Ok(())
    }
}

fn declaration<'a, 'c: 'a>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'a, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devout!("----------------------------");
    devnote!(this it "declaration");

    // Reset expression tracking for each declaration
    this.last_was_expression = false;

    let t = this.peek(it)?;
    match t {
        Token::Local => declaration_keyword(this, mc, f, it, true, false)?,
        Token::Global => declaration_keyword(this, mc, f, it, false, false)?,
        Token::Function => {
            this.eat(it);
            define_function(this, mc, f, it, true, None)?;
        }
        _ => statement(this, mc, f, it)?,
    }
    Ok(())
}

fn declaration_keyword<'a, 'c: 'a>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'a, 'c>,
    it: &mut Peekable<Lexer>,
    local: bool,
    already_function: bool,
) -> Catch {
    devnote!(this it "declaration_keyword");
    this.eat(it);
    let (res, location) = this.pop(it);
    match res? {
        Token::Identifier(ident) => {
            if this.scope_depth > 0 && local {
                //local
                //TODO should we warn? redefine_behavior(this,ident)?
                add_local(this, it, ident)?;
                typing(this, f, it, None)?;
            } else {
                let ident = this.identifer_constant(f, ident);
                typing(this, f, it, Some((ident, location)))?;
            }
        }
        Token::Function => {
            if !already_function {
                define_function(this, mc, f, it, local, None)?;
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

fn declaration_scope<'a, 'c: 'a>(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    ident: String,
    local: bool,
    location: TokenCell,
) -> Catch {
    if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this, it, ident)?;
        typing(this, f, it, None)?;
    } else {
        let ident = this.identifer_constant(f, ident);
        typing(this, f, it, Some((ident, location)))?;
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
fn add_local(
    this: &mut Compiler,
    it: &mut Peekable<Lexer>,
    ident: String,
) -> Result<u8, ErrorTuple> {
    _add_local(this, it, Some(ident))
}

/** Store location on the stack with a placeholder that cannot be resolved as a variable, only reserves for operations */
fn add_local_placeholder(this: &mut Compiler, it: &mut Peekable<Lexer>) -> Result<u8, ErrorTuple> {
    _add_local(this, it, None)
}

fn _add_local(
    this: &mut Compiler,
    it: &mut Peekable<Lexer>,
    ident: Option<String>,
) -> Result<u8, ErrorTuple> {
    devnote!(this it "add_local");
    // let offset = if this.functional_depth > 0 {
    //     this.local_functional_offset[this.functional_depth - 1]
    // } else {
    //     0
    // };
    let i = this.local_count; //- offset;
    if i == 255 {
        return Err(this.error_at(SiltError::TooManyLocals));
    }
    this.locals.push(Local {
        ident,
        depth: this.scope_depth,
        functional_depth: this.functional_depth,
        is_captured: false,
    });
    this.local_count += 1;
    // let offset = if this.functional_depth > 0 {
    //     this.local_functional_offset[this.functional_depth - 1]
    // } else {
    //     0
    // };
    Ok(i as u8)
}

// /** Remove a single reserved local */
// fn pop_local(this: &mut Compiler) {
//     devnote!(this "pop_local");
//     this.local_count -= 1;
//     this.locals.pop();
// }

fn resolve_local(
    this: &mut Compiler,
    it: &mut Peekable<Lexer>,
    ident: &String,
) -> Option<(u8, bool)> {
    devnote!(this it "resolve_local");
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

fn typing<'a, 'c: 'a>(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    ident_tuple: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this it "typing");
    if let Token::Colon = this.peek(it)? {
        // typing or self calling
        this.eat(it);
        this.store(it);
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
            define_declaration(this, f, it, ident_tuple)?;
        } else {
            todo!("typing");
            // self.error(SiltError::InvalidColonPlacement);
            // Statement::InvalidStatement
        }
    } else {
        define_declaration(this, f, it, ident_tuple)?;
    }
    Ok(())
}

fn define_declaration<'a, 'c: 'a>(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    ident_tuple: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this it "define_declaration");
    this.store(it);
    let t = this.get_current()?;
    match t {
        Token::Assign => {
            expression(this, f, it, false)?;
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
        _ => this.emit_at(f, OpCode::NIL), // TODO are more then just declarations hitting this syntactic sugar?
    }
    define_variable(this, it, f, ident_tuple)?;
    Ok(())
}

fn define_variable<'a, 'c: 'a>(
    this: &mut Compiler,
    it: &mut Peekable<Lexer>,
    f: FnRef,
    ident: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this it "define_variable");

    if let Some(ident) = ident {
        this.emit(f, OpCode::DEFINE_GLOBAL { constant: ident.0 }, ident.1);
    }
    Ok(())
}

fn define_function<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    local: bool,
    pre_ident: Option<usize>,
) -> Catch {
    let (ident, location) = if let &Token::Identifier(_) = this.peek(it)? {
        let (res, location) = this.pop(it);
        if let Token::Identifier(ident) = res? {
            (ident, location)
        } else {
            unreachable!()
        }
    } else {
        ("anonymous".to_string(), this.current_location)
    };

    let ident_clone = ident.clone();
    let global_ident = if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this, it, ident)?;
        None
    } else {
        Some((this.identifer_constant(f, ident), location))
    };

    build_function(this, mc, f, it, ident_clone, global_ident, false)?;

    Ok(())
}

/** builds function, implicit return specifices whether a nil is return or the last value popped */
fn build_function<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    ident: String,
    global_ident: Option<(u8, TokenCell)>,
    is_script: bool,
) -> Catch {
    // TODO this function could be called called rercursivelly due to the recursive decent nature of the parser, we should add a check to make sure we don't overflow the stack
    devnote!(this it "build_function");
    let mut f2 = FunctionObject::new(Some(ident), is_script);
    let fr2 = &mut f2;
    // this.swap_function(&mut sidelined_func);
    // swap(f, &mut sidelined_func);
    begin_scope(this);
    begin_functional_scope(this);
    expect_token!(this it OpenParen);
    let mut arity = 0;
    if let Token::Identifier(_) = this.peek(it)? {
        arity += 1;
        build_param(this, it)?;

        while let Token::Comma = this.peek(it)? {
            this.eat(it);
            arity += 1;
            if arity > 255 {
                // TODO we should use an arity value on the function object but let's make it only exist on compile time
                return Err(this.error_at(SiltError::TooManyParameters));
            }
            build_param(this, it)?;
        }
    }

    // this.override_pop=true; // the function declare is inside our scope and it would trigger a pop
    block(this, mc, fr2, it)?;

    if let &OpCode::RETURN(_) = fr2.chunk.code.last().unwrap() { //read_last_code
    } else {
        this.drop_last_if(fr2, &OpCode::POP);
        // println!("impli {}",implicit_return);
        // TODO if last was semicolon we also push a nil
        // Check if implicit returns are enabled and last statement was an expression
        if this.language_flags.implicit_returns && this.last_was_expression {
            // Don't emit NIL, the last expression value(s) are already on the stack
            // If we have multiple expressions, they're all on the stack for multiple returns
        } else {
            this.emit_at(fr2, OpCode::NIL);
        }
        print_var_stack(&this.var_stack);
        devout!(
            "{} {}",
            "=============================== return is".purple(),
            this.expression_count
        );
        this.emit_at(fr2, OpCode::RETURN(this.expression_count));
        this.expression_count = 0;
    }

    // TODO why do we need to eat again? This prevents an expression_statement of "End" being called but block should have eaten it?
    // if let Token::End = this.peek()? {
    //     this.eat();
    // }

    end_scope(this, fr2, true);
    let upvals = end_functional_scope(this);
    // When we're done compiling the function object we drop the current body function back in and push the compiled func as a constant within that body
    // this.swap_function(&mut sidelined_func);
    // swap(f, &mut sidelined_func);
    f2.upvalue_count = upvals.len() as u8;
    let func_value = Value::Function(Gc::new(mc, f2));
    if true {
        // need closure
        let constant = f.chunk.write_constant(func_value) as u8;
        this.emit_at(f, OpCode::CLOSURE { constant });
        // emit upvalues

        for val in upvals.iter() {
            // TODO is it worth specifying difference between first function enclosure from higher functional enclosure?
            this.emit_at(
                f,
                OpCode::REGISTER_UPVALUE {
                    index: val.ident,
                    neighboring: val.neighboring,
                },
            );
        }
    } else {
        // no closure needed
        // this.constant_at(f, func_value);
    }
    define_variable(this, it, f, global_ident)?;

    Ok(())
}

fn build_param(this: &mut Compiler, it: &mut Peekable<Lexer>) -> Catch {
    let (res, _) = this.pop(it);
    match res? {
        Token::Identifier(ident) => {
            add_local(this, it, ident)?;
        }
        _ => {
            return Err(this.error_at(SiltError::ExpectedLocalIdentifier));
        }
    }
    Ok(())
}

fn statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "statement");

    // Most statements are not expressions, so reset the flag
    this.last_was_expression = false;
    this.expression_count = 1;

    match this.peek(it)? {
        Token::Print => print(this, f, it)?,
        Token::If => if_statement(this, mc, f, it)?,
        Token::Do => {
            this.eat(it);
            begin_scope(this);
            block(this, mc, f, it)?;
            end_scope(this, f, false);
        }
        Token::While => while_statement(this, mc, f, it)?,
        Token::For => for_statement(this, mc, f, it)?,
        Token::Return => return_statement(this, f, it)?,
        // Token::OpenBrace => block(this),
        Token::ColonColon => set_goto_label(this, f, it)?,
        Token::Goto => goto_statement(this, f, it)?,
        Token::SemiColon => {
            this.eat(it);
            // TODO ???
        }
        // Token::End => {
        //     // this.eat();
        //     // TODO ???
        // }
        _ => expression_statement(this, f, it)?, // This will set last_was_expression = true
    }
    Ok(())
}

fn block<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "block");
    build_block_until_then_eat!(this, mc, f, it, End);

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
    let cumulative: usize = this.local_functional_offset.iter().sum();
    this.local_functional_offset
        .push(this.local_count + cumulative - 1);
    this.local_offset.push(this.local_count);
    this.local_count = 1;
}

/** raise scope depth and dump lowest locals off the imaginary stack */
fn end_scope(this: &mut Compiler, f: FnRef, skip_code: bool) {
    this.scope_depth -= 1;

    let mut last_was_pop = true;
    let mut count = 0;
    let mut v = vec![];
    while !this.locals.is_empty() && this.locals.last().unwrap().depth > this.scope_depth {
        let l = this.locals.pop().unwrap();
        this.local_count -= 1;
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
            this.emit_at(f, OpCode::POPS(*c));
        } else {
            this.emit_at(f, OpCode::CLOSE_UPVALUES(*c));
        }
    });
}

/** raise functional depth */
fn end_functional_scope(this: &mut Compiler) -> Vec<UpLocal> {
    this.functional_depth -= 1;
    this.local_functional_offset.pop();
    this.local_count = match this.local_offset.pop() {
        Some(v) => v,
        None => 1,
    };

    this.up_values.pop().unwrap()
}

fn if_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "if_statement");
    this.eat(it);
    expression(this, f, it, false)?;
    expect_token!(this it Then);
    let skip_if = this.emit_index(f, OpCode::POP_AND_GOTO_IF_FALSE(0));
    scope_and_block_until!(this, mc, f, it, End | Else | ElseIf);
    // this.emit_at(OpCode::POP); // pop the if compare again as we skipped the pop from before
    match this.peek(it)? {
        Token::Else => {
            this.eat(it);
            let skip_else = this.emit_index(f, OpCode::FORWARD(0));
            this.patch(f, skip_if)?; // patch to fo AFTER the forward so we actually run the else block
            scope_and_block_until!(this, mc, f, it, End);
            this.patch(f, skip_else)?;
            expect_token!(this it End);
        }
        Token::ElseIf => {
            this.eat(it);
            // this.emit_at(OpCode::POP);
            this.patch(f, skip_if)?;
            if_statement(this, mc, f, it)?;
        }
        _ => {
            this.patch(f, skip_if)?;
        }
    }
    Ok(())
}

fn while_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "while_statement");
    this.eat(it);
    let loop_start = this.get_chunk_size(f);
    expression(this, f, it, false)?;
    expect_token!(this it Do);
    let exit_jump = this.emit_index(f, OpCode::POP_AND_GOTO_IF_FALSE(0));
    build_block_until_then_eat!(this, mc, f, it, End);
    this.emit_rewind(f, loop_start);
    this.patch(f, exit_jump)?;
    Ok(())
}

/**
 * Put iterator, start, and either step expression or 1 constant on to stack.
 * Evaluate if iterator is less than or equal to end value, if not run block and push iterator to that block's stack
 * At end of block increment iterator by step value and rewind
 * Re-evaluate and if iterator is greater than end value, forward to immediately after end of block
 */
fn for_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "for_statement");
    this.eat(it);
    let pair = this.pop(it);
    let t = pair.0?;
    if let Token::Identifier(ident) = t {
        // let offset = this.local_functional_offset[this.functional_depth - 1];
        let iterator = add_local_placeholder(this, it)?; // reserve iterator with placeholder
        expect_token!(this it Assign);
        add_local_placeholder(this, it)?; // reserve end value with placeholder
        add_local_placeholder(this, it)?; // reserve step value with placeholder
        expression(this, f, it, false)?; // expression for iterator
        expect_token!(this it Comma);
        expression(this, f, it, false)?; // expression for end value

        // let exit_jump = this.emit_index(OpCode::GOTO_IF_FALSE(0));
        // this.emit_at(OpCode::POP);
        // either we have an expression for the step or we set it to 1i
        if let Token::Comma = this.peek(it)? {
            this.eat(it);
            expression(this, f, it, false)?;
        } else {
            this.constant_at(f, Value::Integer(1))
        };
        let for_start = this.emit_index(f, OpCode::FOR_NUMERIC(0));
        // this.emit_at(OpCode::GET_LOCAL { index: iterator });
        // let loop_start = this.get_chunk_size();
        // compare iterator to end value
        // this.emit_at(OpCode::GET_LOCAL { index: comparison });
        // this.emit_at(OpCode::EQUAL);
        // let exit_jump = this.emit_index(OpCode::GOTO_IF_TRUE(0));
        // this.emit_at(OpCode::POP);
        expect_token!(this it Do);
        begin_scope(this);
        add_local(this, it, ident)?; // we add the local inside the scope which was actually added on by the for opcode already
        build_block_until_then_eat!(this, mc, f, it, End);
        end_scope(this, f, false);

        this.emit_at(f, OpCode::INCREMENT { index: iterator });
        this.emit_rewind(f, for_start);
        this.patch(f, for_start)?;
        this.force_stack_pop(f, 3);
        Ok(())
    } else {
        Err(this.error_at(SiltError::ExpectedLocalIdentifier))
    }
}

/**
 * We run closure and if value is not nil we set that to iterator and push onto blocks scope, when we hit end we rewind and re-eval
 * If the for's iterator is nil we forward to end of do block and pop off the iterator
 */
fn generic_for_statement() {}

fn return_statement(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "return_statement");
    devout!("{} {}", "HERE".on_red(), this.expression_count);
    this.eat(it);
    if let Token::End | Token::Else | Token::ElseIf | Token::SemiColon | Token::EOF =
        this.peek(it)?
    {
        this.emit_at(f, OpCode::NIL);
    } else {
        expression(this, f, it, false)?;
        // expression() will set this.expression_count to the number of comma-separated expressions
    }

    // For multiple return values, all expressions are already on the stack
    this.emit_at(f, OpCode::RETURN(this.expression_count));
    Ok(())
}

fn set_goto_label(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "goto_label");
    this.eat(it);
    let token = this.pop(it).0?;
    if let Token::Identifier(ident) = token {
        this.labels.insert(ident, this.get_chunk_size(f));
    } else {
        return Err(this.error_at(SiltError::ExpectedLabelIdentifier));
    }
    Ok(())
}

fn goto_statement(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "goto_statement");
    this.eat(it);
    let token = this.pop(it).0?;
    if let Token::Identifier(ident) = token {
        let size = this.get_chunk_size(f);
        resolve_goto(this, f, &*ident, size, None)?;
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
    f: FnRef,
    ident: &str,
    op_count: usize,
    replace: Option<(usize, TokenCell)>,
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
                    this.change_code(f, i, code);
                }
                None => {
                    this.emit_at(f, code);
                }
            }
        }
        None => match replace {
            Some((i, location)) => {
                return Err(this.error_syntax(SiltError::UndefinedLabel(ident.to_owned()), location))
            }
            None => {
                let index = this.emit_index(f, OpCode::FORWARD(0));
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

fn goto_scope_skip(this: &mut Compiler, f: FnRef) {
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

    this.emit_at(f, OpCode::POPS(i));
}

fn expression(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, skip_step: bool) -> Catch {
    devnote!(this it "expression");
    this.parse_precedence(f, it, Precedence::Assignment, skip_step)?;

    while let Token::Comma = this.peek(it)? {
        add!(this);
        println!("COMMAS");
        this.eat(it);
        println!("===================exp count {}", this.expression_count);
        this.parse_precedence(f, it, Precedence::Assignment, false)?;
    }

    Ok(())
}

/// Walk through expression precedence but stop at commas, used by arguments, and table building
fn expression_single(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    skip_step: bool,
) -> Catch {
    devnote!(this it "expression_single");
    this.parse_precedence(f, it, Precedence::Assignment, skip_step)?;
    Ok(())
}

fn next_expression(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "next_expression");
    this.eat(it);
    expression(this, f, it, false)?;
    Ok(())
}

fn expression_statement(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "expression_statement");
    devout!(
        "{} {}",
        "At (expression statement start)".on_cyan(),
        this.expression_count
    );

    expression(this, f, it, false)?;

    // Mark that the last statement was an expression for implicit returns
    this.last_was_expression = true;

    if this.override_pop {
        this.override_pop = false;
    } else {
        // For implicit returns, we might want to keep the value(s) on the stack
        // if this is the last statement in a function, but we can't know that here
        // The function compilation will handle this by checking last_was_expression

        // If we have multiple expressions and implicit returns are enabled,
        // we might want to keep them all on the stack for the function to return
        if this.language_flags.implicit_returns && this.expression_count > 1 {
            // Don't pop - keep all values for potential multiple return
        } else {
            // Pop the single expression value as usual
            this.emit_at(f, OpCode::POP);
        }
    }
    devnote!(this it "expression_statement end");
    Ok(())
}

fn variable(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, can_assign: bool) -> Catch {
    devnote!(this it "variable");
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

    named_variable(this, f, it, can_assign)?;
    Ok(())
}

fn resolve_etters(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    ident: String,
) -> (OpCode, OpCode) {
    // TODO currently this mechanism searches the entire local stack to determine local and then up values,  ideally we check up values first once we raise out of the functional scope instead of continuing to walk the local stack, but this will work for now.
    match resolve_local(this, it, &ident) {
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
            // println!("============== we in {}", ident);
            let ident = this.identifer_constant(f, ident);
            // add_upvalue(this, ident, this.scope_depth);
            (
                OpCode::SET_GLOBAL { constant: ident },
                OpCode::GET_GLOBAL { constant: ident },
            )
        }
    }
}

fn print_var_stack(_v: &[(OpCode, OpCode)]) {
    #[cfg(feature = "dev-out")]
    {
        println!(":::::::::::::::::::::::::::::::::::::");
        print!("var stack -> ");
        for v in _v.iter() {
            print!("({},{})", v.0, v.1)
        }
        println!();
        println!(":::::::::::::::::::::::::::::::::::::");
    }
}

fn named_variable(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    can_assign: bool,
) -> Catch {
    devnote!(this it "-> named_variables");
    let t = this.copy_store()?;

    // declare shortcut only valid if we're above scope 0 otherwise it's redundant
    // TODO should we send a warning that 0 scoped declares are redundant?
    if this.scope_depth > 0
        && if let Token::Op(Operator::ColonEquals) = this.peek(it)? {
            true
        } else {
            false
        }
    {
        if let Token::Identifier(ident) = t {
            // short declare
            add_local(this, it, ident)?;
            this.override_pop = true;
            this.eat(it);
            expression(this, f, it, false)?;
        } else {
            unreachable!()
        }
        return Ok(());
    }

    // ident getter/setter gather for 1 variable, then continue on our while loop to check for
    // more. This should usually only hit for multi var assignment

    let ops = if let Token::Identifier(ident) = t {
        // devout!("assigning to identifier: {}", ident);
        resolve_etters(this, f, it, ident)
    } else {
        unreachable!()
    };

    this.var_stack.push(ops);
    // Check for additional variables in multi-variable context
    while let Token::Comma = this.peek(it)? {
        add!(this);
        this.eat(it);
        if let Token::Identifier(_) = this.peek(it)? {
            let t = this.pop(it);
            this.current_location = t.1;

            let ops = if let Token::Identifier(ident) = t.0? {
                resolve_etters(this, f, it, ident)
            } else {
                unreachable!()
            };
            this.var_stack.push(ops);
        } else {
            // We encountered a non-identifier after comma
            // This means we have a mixed expression like "a, 5" or "a, func()"
            // For assignment, this is invalid. For retrieval, we need to handle it differently.
            let t = this.peek(it)?;

            if can_assign && matches!(t, Token::Assign) {
                // This would be invalid assignment like "a, 5 = ..."
                return Err(this.error_at(SiltError::InvalidAssignment(t.clone())));
            }

            // For retrieval context, we need to drain the getters we've collected so far
            // and then continue parsing as a regular expression
            // this.return_count = this.var_stack.len() as u8;
            this.drain_getters(f);

            // Now parse the remaining expression starting from current position
            // We need to handle this as part of a larger comma-separated expression
            // this.return_count += 1;
            this.parse_precedence(f, it, Precedence::Assignment, false)?;

            return Ok(());
        }
    }
    print_var_stack(&this.var_stack);
    // loop {
    //     //normal assigment
    //
    //     devout!("setter: {}, getter: {}", setter, getter);
    //
    //     if let  {
    //     } else {
    //         break;
    //     }
    // }
    //

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
    match this.peek(it)? {
        Token::Assign => {
            if can_assign {
                this.eat(it);
                let assign_need = this.var_stack.len() as isize;
                this.expression_count = 1;
                // if assign_need > 1 {
                //     this.emit_at(f, OpCode::NEED(assign_need as u8));
                // }
                // println!("=============== pre setters {}", this.peek(it)?);
                std::mem::swap(&mut this.var_stack, &mut this.var_set_stack);
                this.override_pop = true;
                expression(this, f, it, false)?;
                // println!("=============== setters? {}", this.var_stack.len());
                print_var_stack(&this.var_set_stack);
                print_var_stack(&this.var_stack);
                // println!(
                //     "{} {} <-> exp# {}",
                //     "==================check here need".yellow(),
                //     assign_need,
                //     this.expression_count
                // );

                // a,b,c,d,e = 1, fn(), fn()
                // 5 = 1, 2 , 3..
                let remainder = assign_need - this.expression_count as isize;
                // println!("reaminder is {}", remainder);
                match remainder.cmp(&0) {
                    Ordering::Greater => {
                        // we have room so spread the last if possible
                        // let offset = this.current_index - 1;
                        match f.chunk.read_last_code() {
                            OpCode::CALL(u, _) => {
                                // the remainder is how much MORE we would need, at least 1 is
                                // already assumed so we add 1+remainder
                                // println!("{} {}", "modify call to ".red(), remainder + 1);
                                f.chunk.patch_last(OpCode::CALL(*u, (remainder + 1) as u8));
                            }
                            _ => this.emit_at(f, OpCode::NILS(remainder as u8)),
                        }
                    }
                    Ordering::Less => {
                        // pop extra
                        this.emit_at(f, OpCode::POPS((-remainder) as u8));
                    }
                    Ordering::Equal => {}
                }
                // for _ in 0..remainder {
                //     this.emit_at(f, OpCode::NIL);
                // }

                this.drain_setters(f);
            } else {
                // this.return_count = this.var_stack.len() as u8;
                this.drain_getters(f);
            }
        }
        Token::OpenBracket | Token::Dot => {
            this.drain_getters(f); // TODO we should probably error if this is higher then 1
            let count = table_indexer(this, f, it)? as u8;
            if let Token::Assign = this.peek(it)? {
                this.eat(it);
                expression(this, f, it, false)?;
                this.emit_at(f, OpCode::TABLE_SET { depth: count });
                // override statement end pop because instruction takes care of it
                this.override_pop = true;
            } else {
                this.emit_at(f, OpCode::TABLE_GET { depth: count });
                // add!(this);
            }
        }
        _ => {
            // this.return_count = this.var_stack.len() as u8;
            this.drain_getters(f);
        }
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

fn grouping(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "-> grouping");
    expression(this, f, it, false)?;
    //TODO expect
    // expect_token!(self, CloseParen, SiltError::UnterminatedParenthesis(0, 0));
    // self.consume(TokenType::RightParen, "Expect ')' after expression.");
    Ok(())
}

fn tabulate(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "-> tabulate");
    this.emit_at(f, OpCode::NEW_TABLE);
    // not immediately closed
    if !matches!(this.peek(it)?, &Token::CloseBrace) {
        let mut count = 0;
        // check if index provided via brackets or ident, otherwise increment our count to build array at the end
        while {
            let start_brace = this.current_location;
            if match this.peek(it)? {
                Token::Identifier(_) => {
                    this.store(it);
                    if let Token::Assign = this.peek(it)? {
                        let ident = this.get_current()?.unwrap_identifier();
                        this.emit_identifer_constant_at(f, ident.clone());
                        this.eat(it);
                        true
                    } else {
                        expression_single(this, f, it, true)?; // we skip the store because the ip is already where it needs to be
                        false
                    }
                }
                Token::OpenBracket => {
                    this.eat(it);
                    expression_single(this, f, it, false)?;
                    expect_token!(
                        this,
                        it,
                        CloseBracket,
                        this.error_at(SiltError::UnterminatedBracket(start_brace.0, start_brace.1))
                    );
                    expect_token!(this, it, Assign, this.error_at(SiltError::ExpectedAssign));
                    true
                }
                _ => {
                    expression_single(this, f, it, false)?; // normal store expression
                    false
                }
            } {
                expression_single(this, f, it, false)?;
                this.emit_at(f, OpCode::TABLE_INSERT { offset: count });
            } else {
                count += 1;
            }

            match this.peek(it)? {
                Token::Comma => {
                    this.eat(it);
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
            this.emit_at(f, OpCode::TABLE_BUILD(count));
        }
    }

    expect_token!(
        this,
        it,
        CloseBrace,
        this.error_at(SiltError::TableExpectedCommaOrCloseBrace)
    );
    Ok(())
}

/** op unary or primary */
fn unary(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "unary");
    let t = this.copy_store()?;
    // self.expression();

    this.parse_precedence(f, it, Precedence::Unary, false)?;
    match t {
        Token::Op(Operator::Sub) => this.emit_at(f, OpCode::NEGATE),
        Token::Op(Operator::Not) => this.emit_at(f, OpCode::NOT),
        Token::Op(Operator::Length) => this.emit_at(f, OpCode::LENGTH),
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

/// Walk down multiple table fields if necessary table1.table2.table3.field
fn table_indexer(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
) -> Result<usize, ErrorTuple> {
    let mut count = 0;
    while match this.peek(it)? {
        Token::OpenBracket => {
            this.eat(it);
            expression(this, f, it, false)?;
            expect_token!(
                this,
                it,
                CloseBracket,
                this.error_at(SiltError::UnterminatedBracket(0, 0))
            );
            true
        }
        Token::Dot => {
            this.eat(it);
            let t = this.pop(it);
            this.current_location = t.1;
            let field = t.0?;
            devout!("table_indexer: {} p:{}", field, this.peek(it)?);
            if let Token::Identifier(ident) = field {
                this.emit_identifer_constant_at(f, ident);
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

fn binary(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "binary");
    let t = this.copy_store()?;
    let l = this.current_location;
    let rule = Compiler::get_rule(&t);
    this.parse_precedence(f, it, rule.precedence.next(), false)?;
    if let Token::Op(op) = t {
        match op {
            Operator::Add => this.emit(f, OpCode::ADD, l),
            Operator::Sub => this.emit(f, OpCode::SUB, l),
            Operator::Multiply => this.emit(f, OpCode::MULTIPLY, l),
            Operator::Divide => this.emit(f, OpCode::DIVIDE, l),

            Operator::Concat => this.emit(f, OpCode::CONCAT, l),

            // Operator::Modulus => self.emit(OpCode::MODULUS, t.1),
            // Operator::Equal => self.emit(OpCode::EQUAL, t.1),
            Operator::Equal => this.emit(f, OpCode::EQUAL, l),
            Operator::NotEqual => this.emit(f, OpCode::NOT_EQUAL, l),
            Operator::Less => this.emit(f, OpCode::LESS, l),
            Operator::LessEqual => this.emit(f, OpCode::LESS_EQUAL, l),
            Operator::Greater => this.emit(f, OpCode::GREATER, l),
            Operator::GreaterEqual => this.emit(f, OpCode::GREATER_EQUAL, l),

            _ => todo!(),
        }
    }
    Ok(())
}

fn concat(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "concat_binary");
    let t = this.copy_store()?;
    let l = this.current_location;
    let rule = Compiler::get_rule(&t);
    this.parse_precedence(f, it, rule.precedence.next(), false)?;

    if let Token::Op(op) = t {
        match op {
            Operator::Concat => this.emit(f, OpCode::CONCAT, l),
            _ => todo!(),
        }
    }
    Ok(())
}

fn and(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "and");
    let index = this.emit_index(f, OpCode::GOTO_IF_FALSE(0));
    this.emit_at(f, OpCode::POP);
    this.parse_precedence(f, it, Precedence::And, false)?;
    this.patch(f, index)?;
    Ok(())
}

fn or(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "or");

    // the goofy way
    // let index = this.emit_index(OpCode::GOTO_IF_FALSE(0));
    // let final_index = this.emit_index(OpCode::GOTO(0));
    // this.patch(index)?;
    // this.emit_at(OpCode::POP);
    // this.parse_precedence(Precedence::Or, false)?;
    // this.patch(final_index)?;

    let index = this.emit_index(f, OpCode::GOTO_IF_TRUE(0));
    this.emit_at(f, OpCode::POP);
    this.parse_precedence(f, it, Precedence::Or, false)?;
    this.patch(f, index)?;
    Ok(())
}

fn integer(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "integer");
    let t = this.copy_store()?;
    let value = if let Token::Integer(i) = t {
        Value::Integer(i)
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn number(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "number");
    let t = this.copy_store()?;
    let value = if let Token::Number(n) = t {
        Value::Number(n)
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn string(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "string");
    let t = this.copy_store()?;
    let value = if let Token::StringLiteral(s) = t {
        Value::String(s.into_string())
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn literal(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "literal");
    let t = this.copy_store()?;
    match t {
        Token::Nil => this.emit_at(f, OpCode::NIL),
        Token::True => this.emit_at(f, OpCode::TRUE),
        Token::False => this.emit_at(f, OpCode::FALSE),
        _ => unreachable!(),
    }
    Ok(())
}

fn call(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(this it "call");
    // let t = this.take_store()?;
    // let l = this.current_location;
    // let rule = Compiler::get_rule(&t);
    // this.parse_precedence(rule.precedence.next(), false)?;
    // if let Token::OpenParen = t {
    //     let arg_count = this.argument_list()?;
    //     this.emit(OpCode::CALL { arg_count }, l);
    // }
    let start = this.current_location;

    // println!("{} ", "TIME TO COUNT".on_cyan());
    let arg_count = arguments(this, f, it, start)?;
    // println!("{} {}", "ARG COUNT".on_cyan(), arg_count);
    this.emit(f, OpCode::CALL(arg_count, 0), start);
    Ok(())
}

fn call_table(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    todo!();
    Ok(())
}

fn call_string(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    todo!();
    Ok(())
}

fn arguments(
    this: &mut Compiler,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    start: TokenCell,
) -> Result<u8, ErrorTuple> {
    devnote!(this it "arguments");
    let mut args = 0;
    if !matches!(this.peek(it)?, &Token::CloseParen) {
        while {
            expression_single(this, f, it, false)?;
            args += 1;
            if let &Token::Comma = this.peek(it)? {
                this.eat(it);
                true
            } else {
                false
            }
        } {
            if args == 255 {
                return Err(this.error_at(SiltError::TooManyParameters));
            }
        }
    }

    expect_token!(
        this,
        it,
        CloseParen,
        this.error_at(SiltError::UnterminatedParenthesis(start.0, start.1))
    );
    devout!("arguments count: {}", args);

    Ok(args)
}


fn print(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "print");
    this.eat(it);
    expression(this, f, it, false)?;
    this.emit_at(f, OpCode::PRINT);
    Ok(())
}

pub fn void(_this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>, _can_assign: bool) -> Catch {
    devnote!(_this it "void");
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
