use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{Display, Formatter},
    iter::Peekable,
    println, vec,
};

use colored::Colorize;
use gc_arena::{Gc, Mutation};

use crate::{
    code::OpCode,
    error::{ErrorOut, ErrorTuple, SiltError, TokenCell, TokenTriple},
    function::FunctionObject,
    lexer::Lexer,
    token::{Operator, Token},
    value::Value,
};

// #[cfg(feature = "dev-out")]
// use colored::Colorize;
#[cfg(feature = "wasm")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

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
            "=>{:?}\t\t//{:?}\t -> current: {}",
            $self.peek($it).unwrap_or(&Token::Nil).clone(),
            $self.get_current().unwrap_or(&Token::Nil),
            $message
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
        $self.increment_expression_count();
        #[cfg(feature = "dev-out")]
        println!("{} {}", "Add".on_cyan(), $self.get_expression_count());
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
    BitOr,      // |
    BitXor,     // ~ (binary)
    BitAnd,     // &
    Shift,      // << >>
    Concat,     // ..
    Term,       // + -
    Factor,     // * / // %
    Unary,      // ~ - !
    Exponent,   // ^ (right-assoc, binds tighter than unary)
    Call,       // . ()
    Primary,
}
// precedence enum includes concat ..

type Ident = u8;

type Catch = Result<(), ErrorTuple>;

/// Toggles for silt's opt-in language extensions. A flag field only exists when
/// its backing cargo feature is compiled in — e.g. `bang_operator` is present
/// only under `feature = "bang"`, and every read of it is behind the same
/// `#[cfg]`. This keeps a disabled feature from leaving dead config surface.
#[derive(Clone, Copy, Debug)]
pub struct LanguageFlags {
    pub implicit_returns: bool,
    /// Arrow functions (`x -> …`). Present only with the `arrow` feature; an
    /// embedder may still toggle it off at runtime.
    #[cfg(feature = "arrow")]
    pub arrow_functions: bool,
    #[cfg(feature = "bang")]
    #[allow(dead_code)]
    pub bang_operator: bool,
    /// Luau-style compound assignment (`+= -= *= /= //= %= ^= ..=`). Present
    /// only with the `compound-assignment` feature; an embedder may still toggle
    /// it off at runtime.
    #[cfg(feature = "compound-assignment")]
    pub compound_assignment: bool,
}

impl Default for LanguageFlags {
    fn default() -> Self {
        Self {
            implicit_returns: false,
            #[cfg(feature = "arrow")]
            arrow_functions: true,
            #[cfg(feature = "bang")]
            bang_operator: false,
            #[cfg(feature = "compound-assignment")]
            compound_assignment: true,
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
            Precedence::Comparison => Precedence::BitOr,
            Precedence::BitOr => Precedence::BitXor,
            Precedence::BitXor => Precedence::BitAnd,
            Precedence::BitAnd => Precedence::Shift,
            Precedence::Shift => Precedence::Concat,
            Precedence::Concat => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Exponent,
            Precedence::Exponent => Precedence::Call,
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
            Precedence::BitOr => write!(f, "BitOr"),
            Precedence::BitXor => write!(f, "BitXor"),
            Precedence::BitAnd => write!(f, "BitAnd"),
            Precedence::Shift => write!(f, "Shift"),
            Precedence::Concat => write!(f, "Concat"),
            Precedence::Term => write!(f, "Term"),
            Precedence::Factor => write!(f, "Factor"),
            Precedence::Unary => write!(f, "Unary"),
            Precedence::Exponent => write!(f, "Exponent"),
            Precedence::Call => write!(f, "Call"),
            Precedence::Primary => write!(f, "Primary"),
        }
    }
}

struct ParseRule {
    prefix: for<'c> fn(
        &mut Compiler,
        mc: &Mutation<'c>,
        f: FnRef<'_, 'c>,
        it: &mut Peekable<Lexer>,
        can_assign: bool,
    ) -> Catch,
    infix: for<'c> fn(
        &mut Compiler,
        mc: &Mutation<'c>,
        f: FnRef<'_, 'c>,
        it: &mut Peekable<Lexer>,
        can_assign: bool,
    ) -> Catch,
    precedence: Precedence,
}

const WORD_MAP: [&str; 7] = [
    "keyword", "op", "number", "bool", "nil", "string", "comment",
];

// type LSPIndent =(usize, usize);
// todo!("make this work");
// todo!("add types feature");
// start, length, type
type LSPFormatMark = (usize, usize, u8);

#[cfg(feature = "wasm")]
#[derive(Serialize, Deserialize)]
pub struct LanguageServerOutput<'c> {
    #[serde(borrow)]
    legend: [&'c str; 7],
    map: Vec<LSPFormatMark>,
    indented: String, // indents: Vec<IndentMark>,
}
#[cfg(not(feature = "wasm"))]
pub struct LanguageServerOutput<'c> {
    legend: [&'c str; 7],
    map: Vec<LSPFormatMark>,
    indented: String, // indents: Vec<IndentMark>,
}

impl Display for LanguageServerOutput<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "legend [{}],\n map ", self.legend.join(","))?;
        for (s, l, t) in self.map.iter() {
            write!(f, "({} {} {}),", s, l, t)?;
        }
        writeln!(f, "{}", self.indented)
    }
}

/** stores a local identifier's name by boxed string, if none is provbided it serves as a placeholder for statements such as a loop, this way they cannot be resolved as variables */
struct Local {
    ident: Option<String>,
    /** scope depth, indepedent of functional depth */
    depth: usize,
    /** how many layers deep the local value is nested in a function, with 0 being global (should only happen once to reserve the root func on the stack) */
    functional_depth: usize,
    is_captured: bool,
    /// Phase-1 static type annotation (compile-time only). Defaults to `Any`.
    /// Only written in Phase 1; the checking phases will read it.
    #[cfg(feature = "typing")]
    #[allow(dead_code)]
    ty: crate::types::Type,
}

/// Bookkeeping for one active loop so `break` can unwind cleanly.
struct LoopCtx {
    /// `local_count` captured before the loop pushed its control/body slots; a
    /// `break` pops `local_count - base_local_count` runtime values to undo them.
    base_local_count: usize,
    /// chunk indices of emitted `FORWARD(0)` break jumps, patched to the loop exit.
    break_jumps: Vec<usize>,
}

struct UpLocal {
    /** location on the overall stack */
    ident: u8,
    /** location on the immediately scoped stack, 1 if it's the first declared value in scope (after closure) */
    // scoped_ident: u8,
    neighboring: bool,
    universal_ident: u8,
}

struct FunctionalState {
    pub up_values: Vec<UpLocal>,
    pub vararg: u8,
    /// we're making a function call or multi assignment and a vararg is at the end
    pub trailing_vararg: bool,
    /// Are we walking arguments for a function call or table build? Changes vararg stack behavior
    pub argument_mode: bool,
    /// tracks the number of values on the stack from comma-separated expressions
    pub expression_count: u8,
    /// hack to flip off a pop when an expression takes on statement properties, used only for := right now
    pub should_override_pop: bool,
}
impl FunctionalState {
    pub fn new() -> Self {
        FunctionalState {
            up_values: vec![],
            vararg: 0,
            trailing_vararg: false,
            argument_mode: false,
            expression_count: 0,
            should_override_pop: false,
        }
    }
}

type FnRef<'a, 'c> = &'a mut FunctionObject<'c>;
#[allow(dead_code)]
type OpOpCode = Option<OpCode>;

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
    functional_states: Vec<FunctionalState>,
    /// hack
    /// tracks argument mode even if outside of functional scope (root level) this is a bit of a
    root_state: FunctionalState,
    local_offset: Vec<usize>,
    /** an offset tracker each time we descend into a new functional scope. For instance if we drop 1 level down from the root level that had 3 locals prior [A,B,C] then our stack looks like [root, A, B, C, fn] then we'll store 3 at this field's index 0 since the calling function is always at the bottom of the stack */
    local_functional_offset: Vec<usize>,
    locals: Vec<Local>,
    local_count: usize,
    /// This statement is a local declaration
    local_declare_mode: bool,
    labels: HashMap<String, usize>,
    // location: (usize, usize),
    // previous: TokenTuple,
    // pre_previous: TokenTuple,
    pending_gotos: Vec<(String, usize, TokenCell)>,
    /// Stack of enclosing loops (innermost last). Each entry records the
    /// `local_count` just before the loop pushed its control/body slots and the
    /// chunk indices of any `break` jumps awaiting a patch to the loop exit.
    loops: Vec<LoopCtx>,
    /// flag that a self calling method was used
    self_arg: bool,
    /** language flags for optional features */
    language_flags: LanguageFlags,
    /** tracks if the last statement was an expression for implicit returns */
    last_was_expression: bool,
    // return_count: u8,
    /// used for multi var assignment, start small, expand if really necessary
    var_stack: Vec<Option<(OpCode, OpCode)>>,
    /// men will do anything to not have to allocate a new vec
    var_set_stack: Vec<Option<(OpCode, OpCode)>>,
    expected_multi: u8,
    // TODO this is a decent stopgap to fix our multivar headaches BUT this will definitely break
    // in our implicit returns as they're treated as setters and wouldnt collapse expressions
    // correctly. If we walk our setters all the way to find an assignment (:=)
    /// can we gather multivars for setters? multivar return or gets must skip this
    can_multivar_set: bool,
    /// Source line of the most recently consumed `end` token (set by `block()`).
    /// Used by `build_function` to record the precise closing line of a function
    /// body for hotswap range detection.
    last_end_line: usize,
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
            functional_states: vec![],
            root_state: FunctionalState::new(),
            locals: vec![Local {
                ident: None,
                depth: 0,
                functional_depth: 0,
                is_captured: false,
                #[cfg(feature = "typing")]
                ty: crate::types::Type::Any,
            }],
            local_functional_offset: vec![],
            local_offset: vec![],
            local_count: 1,
            local_declare_mode: false,
            labels: HashMap::new(),
            pending_gotos: vec![],
            loops: vec![],
            // location: (0, 0),
            // previous: (Token::Nil, (0, 0)),
            // pre_previous: (Token::Nil, (0, 0)),
            self_arg: false,
            language_flags: LanguageFlags::default(),
            last_was_expression: false,
            var_stack: Vec::with_capacity(4),
            var_set_stack: Vec::with_capacity(4),
            expected_multi: 0,
            can_multivar_set: true,
            last_end_line: 0,
        }
    }

    /** Create a new compiler instance with language flags. `bang_operator` is
    ignored unless the `bang` feature is enabled (the field only exists then). */
    #[allow(unused_variables)]
    pub fn new_with_flags(
        implicit_returns: bool,
        arrow_functions: bool,
        bang_operator: bool,
    ) -> Compiler {
        let mut compiler = Self::new();
        compiler.language_flags = LanguageFlags {
            implicit_returns,
            #[cfg(feature = "arrow")]
            arrow_functions,
            #[cfg(feature = "bang")]
            bang_operator,
            ..LanguageFlags::default()
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

    fn set_can_multivar_set(&mut self, value: bool) {
        self.can_multivar_set = value;
        // println!("multi set to {}", value);
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
                (Ok(t.0), (t.1.line, t.1.col))
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
        let _t = iter.next();
        #[cfg(feature = "dev-out")]
        {
            match _t {
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
            Some(Ok(t)) => (Ok(t.0), (t.1.line, t.1.col)),
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
    #[allow(dead_code)]
    fn store_and_return(&mut self, iter: &mut Peekable<Lexer>) -> Result<Token, ErrorTuple> {
        self.current_index += 1;
        let (r, l) = match iter.next() {
            Some(Ok(t)) => (Ok(t.0), (t.1.line, t.1.col)),
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
        let it = vv.peekable();
        for v in it {
            if let Some(s) = v {
                // println!("we writting code here {} {}", v.0, local);
                f.chunk.write_code(s.0, self.current_location);
                // if !local {
                f.chunk.write_code(OpCode::POP, self.current_location);
                // }
            }
        }
    }

    fn drain_getters(&mut self, f: FnRef) {
        devout!("{}", "drain getters".green());
        for v in self.var_stack.drain(..) {
            if let Some(s) = v {
                // println!("⭐⭐⭐DRAIN GETTERS {}",s.1);
                f.chunk.write_code(s.1, self.current_location);
            }
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
    fn peek_triple<'c>(
        &mut self,
        iter: &'c mut Peekable<Lexer>,
    ) -> Result<&'c (Token, TokenTriple), ErrorTuple> {
        match iter.peek() {
            Some(Ok(t)) => {
                // devout!("peek {}", t.0);
                Ok(t)
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
            None => Err(ErrorTuple {
                code: SiltError::Unknown,
                location: (0, 0),
            }),
        }
    }

    /** return the peeked token tuple (token+location) result */
    fn peek_result<'c>(&mut self, iter: &'c mut Peekable<Lexer>) -> Result<&'c Token, ErrorTuple> {
        #[cfg(feature = "dev-out")]
        match iter.peek() {
            Some(r) => match r {
                Ok(t) => {
                    println!("peek_res {}", t.0);
                    Ok(&t.0)
                }
                Err(e) => {
                    println!("peek_res err {}", e.code);
                    Err(e.clone())
                }
            },
            None => Ok(&Token::EOF),
        }
        #[cfg(not(feature = "dev-out"))]
        {
            // let t = iter.peek();
            match iter.peek() {
                Some(r) => match r {
                    Ok(o) => Ok(&o.0),
                    Err(e) => Err(e.clone()),
                },
                None => Ok(&Token::EOF),
            }
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
        let _b = f.chunk.drop_last_if(op);
        #[cfg(feature = "dev-out")]
        {
            println!("{} drop_last_if {}? {}!", "DROP".on_red(), op, _b);
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn is_end(&mut self, iter: &mut Peekable<Lexer>) -> bool {
        match iter.peek() {
            None => true,
            _ => false,
        }
    }

    fn is_vararg_function(&self) -> bool {
        self.functional_depth > 0 && self.functional_states[self.functional_depth - 1].vararg > 0
    }

    fn is_arg_mode(&self) -> bool {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].argument_mode
        } else {
            self.root_state.argument_mode
        }
    }
fn is_trailing_vararg(&self) -> bool {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].trailing_vararg
        } else {
            self.root_state.trailing_vararg
        }
    }
fn set_trailing_vararg(&mut self,bool: bool){
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].trailing_vararg=bool;
        } else {
            self.root_state.trailing_vararg=bool;
        }
    }

    fn set_arg_mode(&mut self, bool: bool) {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].argument_mode = bool;
        } else {
            self.root_state.argument_mode = bool;
        }
    }

    fn set_vararg(&mut self) {
        if self.functional_depth > 0 {
            // println!("{} {}", "Set local count".on_red(), self.local_count);
            self.functional_states[self.functional_depth - 1].vararg = (self.local_count) as u8;
        }
    }
    fn get_vararg(&self) -> u8 {
        if self.functional_depth > 0 {
            return self.functional_states[self.functional_depth - 1].vararg;
        }
        0
    }

    fn set_expression_count(&mut self, val: u8) {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].expression_count = val;
        } else {
            self.root_state.expression_count = val;
        }
    }

    fn get_expression_count(&self) -> u8 {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].expression_count
        } else {
            self.root_state.expression_count
        }
    }

    fn increment_expression_count(&mut self) {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].expression_count += 1;
        } else {
            self.root_state.expression_count += 1;
        }
    }

    fn flip_override_pop(&mut self) -> bool {
        let reff = if self.functional_depth > 0 {
            &mut self.functional_states[self.functional_depth - 1].should_override_pop
        } else {
            &mut self.root_state.should_override_pop
        };
        let res = *reff;
        (*reff) = false;
        res
    }

    fn override_pop(&mut self) {
        if self.functional_depth > 0 {
            self.functional_states[self.functional_depth - 1].should_override_pop = true;
        } else {
            self.root_state.should_override_pop = true;
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
            Token::OpenBrace => rule!(tabulate, void, None),
            Token::Assign => rule!(void, void, None),
            // method call on a non-identifier receiver, e.g. `("hi"):upper()`.
            // Identifier/table receivers are consumed earlier in named_variable.
            Token::Colon => rule!(void, method_infix, Call),
            Token::Op(op) => match op {
                Operator::Sub => rule!(unary, binary, Term),
                Operator::Add => rule!(void, binary, Term),
                Operator::Multiply => rule!(void, binary, Factor),
                Operator::Divide => rule!(void, binary, Factor),
                Operator::Modulus => rule!(void, binary, Factor),
                Operator::FloorDivide => rule!(void, binary, Factor),
                // `^` is right-associative and binds tighter than unary, so it
                // uses a dedicated infix and the Exponent precedence level.
                Operator::Exponent => rule!(void, exponent, Exponent),
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
                Operator::BitOr => rule!(void, binary, BitOr),
                // `~` is unary bitwise-not (prefix) and binary xor (infix)
                Operator::Tilde => rule!(unary, binary, BitXor),
                Operator::BitAnd => rule!(void, binary, BitAnd),
                Operator::ShiftLeft => rule!(void, binary, Shift),
                Operator::ShiftRight => rule!(void, binary, Shift),
                Operator::Length => rule!(unary, void, None),
                _ => rule!(void, void, None),
            },
            Token::Identifier(_) => rule!(variable, void, None),
            Token::Function => rule!(function_expression, void, None),
            Token::VarArg => rule!(vararg_variable, void, None),
            // Token::OpenBracket => rule!(void, indexer, Call),
            Token::Integer(_) => rule!(integer, void, None),
            Token::Number(_) => rule!(number, void, None),
            Token::StringLiteral(_) => rule!(string, void, None),
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
        name: Option<&str>,
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
        let mut body = FunctionObject::new(to_op_string(name), true);
        let mut iter = lexer.peekable();

        while iter.peek().is_some() {
            match declaration(self, mc, &mut body, &mut iter) {
                Ok(()) => {}
                Err(e) => {
                    self.push_error(e);
                    self.synchronize(&mut iter);
                }
            }
        }

        // Handle implicit returns for multiple expressions
        if self.language_flags.implicit_returns && self.last_was_expression {
            // If we have multiple expressions, keep them all on the stack
            // Otherwise, drop the last POP to keep the single expression
            if self.get_expression_count() <= 1 {
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
        name: Option<&str>,
        source: &str,
    ) -> Result<FunctionObject<'c>, ErrorOut> {
        let obj = self.compile(mc, name, source);
        if obj.chunk.is_valid() {
            Ok(obj)
        } else {
            Err(ErrorOut {
                errors: self.pop_errors(),
                source: to_op_string(name),
            })
        }
    }

    pub fn lsp(&mut self, source: &str, format: bool) -> LanguageServerOutput<'_> {
        let mut map = Vec::new();
        let mut indented = String::new();
        let mut indent_level = 0;
        let mut at_line_start = true;
        let mut last_pos = 0;
        let mut offset: isize = 0;

        let lexer = Lexer::new(source);
        let mut iter = lexer.peekable();

        while let Some(token_result) = iter.next() {
            match token_result {
                Ok((
                    token,
                    TokenTriple {
                        line: _,
                        col: _,
                        index,
                        length,
                    },
                )) => {
                    let start = index;
                    // Add any whitespace/newlines between tokens to maintain formatting
                    if start > last_pos && format {
                        let between = &source[last_pos..start];
                        // indented.push_str(between);

                        devout!(">>{}<<", between);
                        // Track if we're at the start of a new line
                        if between.contains('\n') {
                            let squash: String = between
                                .chars()
                                .filter(|c| !c.is_whitespace() || *c == '\n')
                                .collect();
                            offset -= (between.len() as isize) - squash.len() as isize;
                            indented.push_str(&squash);
                            at_line_start = true;
                        } else {
                            indented.push_str(between);
                        }
                    }

                    // Add indentation at the start of new lines
                    if format && at_line_start && !matches!(token, Token::EOF) {
                        // Adjust indent level based on token
                        match &token {
                            Token::End | Token::Else | Token::ElseIf => {
                                if indent_level > 0 {
                                    indent_level -= 1;
                                }
                            }
                            _ => {}
                        }

                        for _ in 0..indent_level {
                            indented.push('\t');
                        }
                        at_line_start = false;
                    }

                    let token_type = match &token {
                        // Keywords (type 1)
                        Token::Local
                        | Token::Global
                        | Token::Function
                        | Token::If
                        | Token::Then
                        | Token::Else
                        | Token::ElseIf
                        | Token::End
                        | Token::While
                        | Token::Do
                        | Token::For
                        | Token::Return
                        | Token::Print
                        | Token::Goto => 1,

                        // Ops (type 2)
                        Token::Op(_) | Token::Assign => 2,

                        // Values (type 3) - numbers
                        Token::Integer(_) | Token::Number(_) => 3,

                        // Values (type 4) - bool
                        Token::True | Token::False => 4,

                        // Values (type 5) - nil
                        Token::Nil => 5,

                        // Strings (type 6)
                        Token::StringLiteral(_) => 6,
                        // Comment (type 7)
                        Token::Comment => 7,

                        // Everything else that's not a comment
                        _ => 0,
                    };

                    // Only add non-whitespace tokens to the map
                    if !matches!(token, Token::EOF) {
                        // println!("start len {} {}", start, length);
                        let token_str = &source[start..start + length];
                        println!("~{}~", token_str);
                        if !token_str.trim().is_empty() {
                            let i: usize = if offset < 0 {
                                start.checked_sub(offset.wrapping_abs() as usize)
                            } else {
                                start.checked_add(offset as usize)
                            }
                            .unwrap_or(0);
                            map.push((i, length, token_type));
                            if format {
                                indented.push_str(token_str);
                            }
                        }
                    }

                    if format {
                        // Adjust indent level for tokens that increase nesting
                        match &token {
                            Token::Then | Token::Do | Token::Function => {
                                indent_level += 1;
                            }
                            _ => {}
                        }
                    }

                    last_pos = start + length;
                }
                Err(_) => {
                    // Skip error tokens but continue processing
                    continue;
                }
            }
        }

        if format {
            // Add any remaining content after the last token
            if last_pos < source.len() {
                indented.push_str(&source[last_pos..]);
            }
        } else {
            indented = source.to_owned();
        }

        LanguageServerOutput {
            legend: WORD_MAP,
            map,
            indented,
        }
    }

    #[cfg(feature = "wasm")]
    #[wasm_bindgen]
    pub fn lsp_wasm(&mut self, source: &str, format: bool) -> String {
        let output = self.lsp(source, format);
        serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn lsp(source: &str, format: bool) -> String {
    let mut compiler = Compiler::new();
    let output = compiler.lsp(source, format);
    serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
}

impl Compiler {
    /// Error recovery. CRITICAL: this must always consume at least one token.
    /// `compile()` loops `while iter.peek().is_some()`, so if a failed
    /// `declaration` left the offending token in place (e.g. an unlexable char
    /// that surfaces as a peek `Err`), the loop would re-parse it forever and
    /// hang. After guaranteeing progress we skip ahead to a likely statement
    /// boundary so a single bad token doesn't cascade into a flood of errors.
    fn synchronize(&mut self, iter: &mut Peekable<Lexer>) {
        self.eat(iter);
        loop {
            let at_boundary = match iter.peek() {
                None => true,
                Some(Ok(tt)) => matches!(
                    &tt.0,
                    Token::Local
                        | Token::Global
                        | Token::Function
                        | Token::If
                        | Token::While
                        | Token::For
                        | Token::Return
                        | Token::Do
                        | Token::End
                        | Token::Print
                        | Token::Goto
                        | Token::ColonColon
                        | Token::SemiColon
                ),
                // a run of unlexable characters: keep eating so we don't spin
                Some(Err(_)) => false,
            };
            if at_boundary {
                break;
            }
            self.eat(iter);
        }
    }

    fn parse_precedence<'c>(
        &mut self,
        mc: &Mutation<'c>,
        f: FnRef<'_, 'c>,
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
        (rule.prefix)(self, mc, f, it, can_assign)?;

        loop {
            let c = self.peek_result(it);
            let rule = match c {
                Ok(&Token::EOF) => break,
                Ok(t) => Self::get_rule(t),
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
            (rule.infix)(self, mc, f, it, false)?;
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
    let t = this.peek(it)?;
    if t == &Token::Comment {
        this.eat(it);
        return Ok(());
    }

    // Reset expression tracking for each declaration
    this.last_was_expression = false;

    match t {
        Token::Local => declaration_keyword(this, mc, f, it, true, false)?,
        Token::Global => declaration_keyword(this, mc, f, it, false, false)?,
        Token::Function => {
            this.eat(it);
            define_function(this, mc, f, it, false, None)?;
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
    this.local_declare_mode = local;
    this.eat(it);
    // let (res, location) = this.pop(it);
    let (r, l) = this.peek_triple(it)?;
    let location = l.clone();
    let res = r.clone();
    match res {
        Token::Identifier(ident) => {
            pre_statement(this);
            if this.scope_depth > 0 && local {
                //local
                //TODO should we warn? redefine_behavior(this,ident)?

                // add_local(this, it, ident)?;
                this.override_pop();
                // this.eat(it);
                typing(this, mc, f, it, None)?;
            } else {
                let ident = this.identifer_constant(f, ident.to_string());
                typing(
                    this,
                    mc,
                    f,
                    it,
                    Some((ident, (location.line, location.col))),
                )?;
                // typing(this, f, it, Some((ident, (location.line,location.col))))?;
            }
        }
        Token::Function => {
            if !already_function {
                this.eat(it);
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

#[allow(dead_code)]
fn declaration_scope<'a, 'c: 'a>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    ident: String,
    local: bool,
    location: TokenCell,
) -> Catch {
    if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this, ident)?;
        typing(this, mc, f, it, None)?;
    } else {
        let ident = this.identifer_constant(f, ident);
        typing(this, mc, f, it, Some((ident, location)))?;
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
    ident: String,
) -> Result<u8, ErrorTuple> {
    _add_local(this,  Some(ident))
}

/** Store location on the stack with a placeholder that cannot be resolved as a variable, only reserves for operations */
fn add_local_placeholder(this: &mut Compiler) -> Result<u8, ErrorTuple> {
    _add_local(this, None)
}

fn _add_local(
    this: &mut Compiler,
    ident: Option<String>,
) -> Result<u8, ErrorTuple> {
    // devnote!(this _it "add_local");
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
        #[cfg(feature = "typing")]
        ty: crate::types::Type::Any,
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
    _it: &mut Peekable<Lexer>,
    ident: &String,
) -> Option<(u8, bool)> {
    // println!("❓resolve_local {}", ident);
    devnote!(this _it "resolve_local");
    devout!("{} {}::{}","resolve stack".on_magenta(),ident,this.locals.iter().map(|l|l.ident.clone().unwrap_or("~".to_string())).collect::<Vec<String>>().join(","));
    for (i, l) in this.locals.iter_mut().enumerate().rev() {
        // println!(
        //     " ⭐ test {} ({}) against {}",
        //     l.ident.clone().unwrap_or("_".to_string()),
        //     i,
        //     ident
        // );
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
                            &mut this.functional_states,
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
    functional_states: &mut Vec<FunctionalState>,
    ident: u8,
    scoped_ident: u8,
    level: usize,
    target: usize,
) -> u8 {
    // `level` is a functional depth (1-based); the matching state lives at `level - 1`.
    let state = &mut functional_states[level - 1];
    let m = &mut state.up_values;
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
        let higher = resolve_upvalue(functional_states, ident, scoped_ident, level - 1, target);
        let state = &mut functional_states[level - 1];
        let m = &mut state.up_values;
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
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _ident_tuple: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this it "typing");
    if let Token::Colon = this.peek(it)? {
        // typing or self calling
        this.eat(it);
        this.store(it);
        let t = this.get_current()?;
        if let Token::ColonIdentifier(_target) = t {
            // method or type name
            todo!("fix this to use new variable parse track");
            // define_declaration(this, mc, f, it, ident_tuple)?;
        } else {
            todo!("typing");
            // self.error(SiltError::InvalidColonPlacement);
            // Statement::InvalidStatement
        }
    } else {
        // println!("we got here {}", ident_tuple.unwrap_or((0, (0, 0))).0);
        expression_statement(this, mc, f, it)?;
        // named_variable(this, f, it, can_assign)?;
        // define_declaration(this, f, it, ident_tuple)?;
    }
    Ok(())
}

#[allow(dead_code)]
fn define_declaration<'a, 'c: 'a>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    ident_tuple: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this it "define_declaration");
    this.store(it);
    let t = this.get_current()?;
    match t {
        Token::Assign => {
            // WRONG We need to just change the entre logic to just use the global var_stack path bt with a cute local guy
            println!("{} {}", "yeaaaaaah".on_magenta(), this.var_stack.len());
            expression_statement(this, mc, f, it)?;
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
    _it: &mut Peekable<Lexer>,
    f: FnRef,
    ident: Option<(Ident, TokenCell)>,
) -> Catch {
    devnote!(this _it "define_variable");

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
    _pre_ident: Option<usize>,
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

    // `function t.a.b()` (field) or `function t:m()` (method). The name is not a
    // plain variable binding — it assigns the closure into an existing table.
    if matches!(this.peek(it)?, Token::Dot | Token::Colon) {
        if local {
            // `local function t:m()` / `local function t.x()` are not valid Lua.
            return Err(this.error_at(SiltError::ExpectedToken(Token::OpenParen)));
        }
        return define_function_member(this, mc, f, it, ident, location);
    }

    let ident_clone = ident.clone();
    let global_ident = if this.scope_depth > 0 && local {
        //local
        //TODO should we warn? redefine_behavior(this,ident)?
        add_local(this,  ident)?;
        None
    } else {
        Some((this.identifer_constant(f, ident), location))
    };

    build_function(this, mc, f, it, ident_clone, false, false, location.0)?;

    define_variable(this, it, f, global_ident)?;

    Ok(())
}

/// Compile `function base.a.b()` / `function base:m()`. We load `base`, walk any
/// intermediate `.field`s with TABLE_GET to reach the owning table, build the
/// closure, and TABLE_SET it into the final key. A `:` separator may only appear
/// before the last name and makes the function a method (implicit `self`). The
/// emitted sequence is stack-neutral: GET base + key constant + CLOSURE (+3) are
/// all consumed by TABLE_SET, so no trailing statement pop is needed.
fn define_function_member<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    base: String,
    location: TokenCell,
) -> Catch {
    devnote!(this it "define_function_member");
    // Push the receiver table.
    let (_setter, getter) = resolve_etters(this, f, it, base);
    this.emit_at(f, getter);

    loop {
        let (sep_res, _) = this.pop(it);
        let is_method = matches!(sep_res?, Token::Colon);

        let (field_res, field_loc) = this.pop(it);
        let field = match field_res? {
            Token::Identifier(s) => s,
            _ => return Err(this.error_at(SiltError::ExpectedFieldIdentifier)),
        };
        this.current_location = field_loc;

        let more = matches!(this.peek(it)?, Token::Dot | Token::Colon);
        if more {
            if is_method {
                // a `:` is only legal immediately before the final name
                return Err(this.error_at(SiltError::ExpectedToken(Token::OpenParen)));
            }
            // descend into the intermediate table
            this.emit_identifer_constant_at(f, field);
            this.emit_at(f, OpCode::TABLE_GET { depth: 1 });
        } else {
            // final component: key, closure, then assign
            this.emit_identifer_constant_at(f, field.clone());
            build_function(this, mc, f, it, field, false, is_method, location.0)?;
            this.emit_at(f, OpCode::TABLE_SET { depth: 1 });
            break;
        }
    }
    Ok(())
}

/** builds function, implicit return specifices whether a nil is return or the last value popped */
fn build_function<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    ident: String,
    is_script: bool,
    is_method: bool,
    start_line: usize,
) -> Catch {
    // TODO this function could be called called rercursivelly due to the recursive decent nature of the parser, we should add a check to make sure we don't overflow the stack
    devnote!(this it "build_function");
    let mut f2 = FunctionObject::new(Some(ident), is_script);
    f2.start_line = start_line;
    let fr2 = &mut f2;
    // this.swap_function(&mut sidelined_func);
    // swap(f, &mut sidelined_func);
    begin_scope(this);
    begin_functional_scope(this);
    let mut arity = 0;
    // Colon-method definition: the implicit `self` occupies the first param slot
    // so the receiver passed by `t:m(...)` (pushed as arg 0 at the call site)
    // lands in it. It is a real local, just not written in the parameter list.
    if is_method {
        add_local(this, "self".to_string())?;
        arity += 1;
    }
    expect_token!(this it OpenParen);
    if let Token::Identifier(_) | Token::VarArg = this.peek(it)? {
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
    // Capture the exact `end` keyword line before emitting any more opcodes.
    // `block()` records the End token's line in `last_end_line` so we can store it
    // in the FunctionObject for hotswap range detection.
    fr2.end_line = this.last_end_line;
    fr2.arity=arity;

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
            this.get_expression_count()
        );
        this.emit_at(fr2, OpCode::RETURN(this.get_expression_count()));
        this.set_expression_count(0);
    }

    // TODO why do we need to eat again? This prevents an expression_statement of "End" being called but block should have eaten it?
    // if let Token::End = this.peek()? {
    //     this.eat();
    // }

    end_scope(this, fr2, true);
    let state = end_functional_scope(this);
    // When we're done compiling the function object we drop the current body function back in and push the compiled func as a constant within that body
    // this.swap_function(&mut sidelined_func);
    // swap(f, &mut sidelined_func);
    f2.upvalue_count = state.up_values.len() as u8;
    f2.is_variadic = state.vararg > 0;
    f2.varidic_index = if state.vararg > 0 {
        state.vararg - 1
    } else {
        0
    };

    let func_value = Value::Function(Gc::new(mc, f2));
    if true {
        // need closure
        let constant = f.chunk.write_constant(func_value) as u8;
        this.emit_at(f, OpCode::CLOSURE { constant });
        // emit upvalues

        for val in state.up_values.iter() {
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

    Ok(())
}

fn build_param(this: &mut Compiler, it: &mut Peekable<Lexer>) -> Catch {
    let (res, _) = this.pop(it);
    match res? {
        Token::Identifier(ident) => {
            add_local(this,  ident)?;
            // typed parameter `function f(a: number)` — record on the param local
            #[cfg(feature = "typing")]
            if matches!(this.peek(it)?, Token::Colon) {
                let ty = parse_type_annotation(this, it)?;
                if let Some(local) = this.locals.last_mut() {
                    local.ty = ty;
                }
            }
        }
        Token::VarArg => {
            if this.is_vararg_function() {
                devout!("{} {}", "TIME TO ERROR".blue(), this.functional_depth);
                return Err(this.error_at(SiltError::InvalidVarArgParam));
            }

            this.set_vararg();
            // NOTE: the `...` parameter deliberately does NOT reserve a local
            // slot. At runtime the variadic overflow lives below the frame base
            // (see CallFrame::get_varargs), so body locals must be numbered
            // contiguously right after the fixed params with no phantom gap.
            // add_local(this,  "...".to_string())?;
        }
        _ => {
            return Err(this.error_at(SiltError::ExpectedLocalIdentifier));
        }
    }
    Ok(())
}

/// set flags to track statement properties, used in statement() and  declaration_keyword()
fn pre_statement(this: &mut Compiler) {
    // Most statements are not expressions, so reset the flag
    this.last_was_expression = false;
    this.set_expression_count(1);
    // we can now set multivars again, x,y=...
    this.set_can_multivar_set(true);
}

fn statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "statement");

    pre_statement(this);

    match this.peek(it)? {
        Token::Print => print(this, mc, f, it)?,
        Token::If => if_statement(this, mc, f, it)?,
        Token::Do => {
            this.eat(it);
            begin_scope(this);
            block(this, mc, f, it)?;
            end_scope(this, f, false);
        }
        Token::While => while_statement(this, mc, f, it)?,
        Token::Repeat => repeat_statement(this, mc, f, it)?,
        Token::Break => break_statement(this, f, it)?,
        Token::For => for_statement(this, mc, f, it)?,
        Token::Return => return_statement(this, mc, f, it)?,
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
        _ => expression_statement(this, mc, f, it)?, // This will set last_was_expression = true
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
    // Inline the block-until-end loop so we can capture the End token's line number
    // directly from the lexer before consuming it.  This gives the hotswap engine the
    // precise source line of the closing `end` keyword for each function body.
    loop {
        match it.peek() {
            Some(Ok((Token::End, triple))) => {
                this.last_end_line = triple.line;
                this.eat(it);
                break;
            }
            Some(Ok((Token::EOF, _))) | None => {
                return Err(this.error_at(SiltError::UnterminatedBlock));
            }
            Some(Err(e)) => {
                let err = ErrorTuple {
                    code: e.code.clone(),
                    location: e.location,
                };
                return Err(err);
            }
            _ => {
                declaration(this, mc, f, it)?;
            }
        }
    }

    Ok(())
}

/** lower scope depth */
fn begin_scope(this: &mut Compiler) {
    this.scope_depth += 1;
}

/** Descend into a function's scope and start a new upvalue vec representing required values a level above us */
fn begin_functional_scope(this: &mut Compiler) {
    this.functional_depth += 1;
    this.functional_states.push(FunctionalState::new());
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
fn end_functional_scope(this: &mut Compiler) -> FunctionalState {
    this.functional_depth -= 1;
    this.local_functional_offset.pop();
    this.local_count = match this.local_offset.pop() {
        Some(v) => v,
        None => 1,
    };

    this.functional_states.pop().unwrap()
}

fn if_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "if_statement");
    this.eat(it);
    expression(this, mc, f, it, false)?;
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
            // Once this branch's block has run, jump over the rest of the
            // elseif/else chain (same role as `skip_else` in the Else arm).
            let skip_chain = this.emit_index(f, OpCode::FORWARD(0));
            this.patch(f, skip_if)?; // false condition -> start of the elseif
            // Do NOT eat the `elseif` token here: the recursive if_statement's
            // own leading eat() consumes it exactly as it would an `if`. Eating
            // it twice would swallow the first token of the elseif condition.
            // The recursion also consumes the single closing `end` for the chain.
            if_statement(this, mc, f, it)?;
            this.patch(f, skip_chain)?; // lands just past the whole chain
        }
        _ => {
            this.patch(f, skip_if)?;
            // a plain `if ... then ... end` must consume its own `end`.
            // build_block_until! stops AT the `end` without eating it; if we
            // leave it, the enclosing block feeds `end` to expression_statement,
            // which emits a stray POP (corrupting a live local) and closes the
            // wrong scope. The Else arm already eats its `end` via expect_token.
            expect_token!(this it End);
        }
    }
    Ok(())
}

/// Record the current `local_count` as a loop's break-unwind base.
fn begin_loop(this: &mut Compiler) {
    this.loops.push(LoopCtx {
        base_local_count: this.local_count,
        break_jumps: vec![],
    });
}

/// Patch every pending `break` to the current position (the loop exit) and pop
/// the loop context.
fn end_loop(this: &mut Compiler, f: FnRef) -> Catch {
    if let Some(ctx) = this.loops.pop() {
        for idx in ctx.break_jumps {
            this.patch(f, idx)?;
        }
    }
    Ok(())
}

/// `break`: unwind the loop's runtime slots and jump (forward) to the loop exit,
/// recorded for patching by `end_loop`.
fn break_statement(this: &mut Compiler, f: FnRef, it: &mut Peekable<Lexer>) -> Catch {
    devnote!(this it "break_statement");
    this.eat(it); // 'break'
    let base = match this.loops.last() {
        Some(c) => c.base_local_count,
        None => return Err(this.error_at(SiltError::InvalidTokenPlacement(Token::Break))),
    };
    let unwind = this.local_count.saturating_sub(base);
    if unwind > 0 {
        this.emit_at(f, OpCode::POPS(unwind as u8));
    }
    let idx = this.emit_index(f, OpCode::FORWARD(0));
    this.loops.last_mut().unwrap().break_jumps.push(idx);
    Ok(())
}

/// `repeat <body> until <cond>` — run the body, then test. The body's scope stays
/// open while `cond` is compiled so `until` can see locals declared in the body.
fn repeat_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "repeat_statement");
    this.eat(it); // 'repeat'
    let loop_start = this.get_chunk_size(f);
    begin_loop(this);
    begin_scope(this);
    let base = this.local_count;
    build_block_until_then_eat!(this, mc, f, it, Until);
    // `until` condition is evaluated with the body's locals still in scope.
    expression(this, mc, f, it, false)?;
    let body_locals = (this.local_count - base) as u8;
    // cond TRUE -> stop (jump to exit); cond FALSE -> repeat. GOTO_IF_TRUE peeks,
    // so each path pops the bool (and any body locals) before continuing.
    let exit_jump = this.emit_index(f, OpCode::GOTO_IF_TRUE(0));
    this.emit_at(f, OpCode::POP); // drop cond (repeat path)
    if body_locals > 0 {
        this.emit_at(f, OpCode::POPS(body_locals));
    }
    this.emit_rewind(f, loop_start);
    this.patch(f, exit_jump)?; // exit path lands here
    this.emit_at(f, OpCode::POP); // drop cond
    if body_locals > 0 {
        this.emit_at(f, OpCode::POPS(body_locals));
    }
    // runtime pops already emitted on both paths; just reconcile compile-time state
    end_scope(this, f, true);
    end_loop(this, f)?;
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
    expression(this, mc, f, it, false)?;
    expect_token!(this it Do);
    let exit_jump = this.emit_index(f, OpCode::POP_AND_GOTO_IF_FALSE(0));
    begin_loop(this);
    // Scope the body so locals declared inside are popped each iteration (before
    // the rewind) instead of leaking and shifting slot indices.
    begin_scope(this);
    build_block_until_then_eat!(this, mc, f, it, End);
    end_scope(this, f, false);
    this.emit_rewind(f, loop_start);
    this.patch(f, exit_jump)?;
    end_loop(this, f)?;
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
        // capture base BEFORE the hidden control slots so `break` unwinds them too
        begin_loop(this);
        let iterator = add_local_placeholder(this)?; // reserve iterator with placeholder
        expect_token!(this it Assign);
        add_local_placeholder(this)?; // reserve end value with placeholder
        add_local_placeholder(this)?; // reserve step value with placeholder
        expression_single(this, mc, f, it, false)?; // expression for iterator
        expect_token!(this it Comma);
        expression_single(this, mc, f, it, false)?; // expression for end value

        // let exit_jump = this.emit_index(OpCode::GOTO_IF_FALSE(0));
        // this.emit_at(OpCode::POP);
        // either we have an expression for the step or we set it to 1i
        if let Token::Comma = this.peek(it)? {
            this.eat(it);
            expression_single(this, mc, f, it, false)?;
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
        add_local(this, ident)?; // we add the local inside the scope which was actually added on by the for opcode already
        build_block_until_then_eat!(this, mc, f, it, End);
        end_scope(this, f, false);

        this.emit_at(f, OpCode::INCREMENT { index: iterator });
        this.emit_rewind(f, for_start);
        this.patch(f, for_start)?;
        this.force_stack_pop(f, 3);
        // break jumps land here, after the hidden control slots are reclaimed
        end_loop(this, f)?;
        Ok(())
    } else {
        Err(this.error_at(SiltError::ExpectedLocalIdentifier))
    }
}

/**
 * We run closure and if value is not nil we set that to iterator and push onto blocks scope, when we hit end we rewind and re-eval
 * If the for's iterator is nil we forward to end of do block and pop off the iterator
 */
#[allow(dead_code)]
fn generic_for_statement() {}

fn return_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    this.set_can_multivar_set(false);
    devnote!(this it "return_statement");
    devout!("{} {}", "HERE".on_red(), this.get_expression_count());
    this.eat(it);
    if let Token::End | Token::Else | Token::ElseIf | Token::SemiColon | Token::EOF =
        this.peek(it)?
    {
        this.emit_at(f, OpCode::NIL);
    } else {
        expression(this, mc, f, it, false)?;
        // expression() will set this.expression_count to the number of comma-separated expressions
    }
    this.set_can_multivar_set(true);

    // For multiple return values, all expressions are already on the stack
    this.emit_at(f, OpCode::RETURN(this.get_expression_count()));
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
            Some((_i, location)) => {
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

#[allow(dead_code)]
fn final_resolve_goto(this: &mut Compiler) {
    this.pending_gotos
        .iter()
        .for_each(|(_ident, _index, _location)| {});
}

#[allow(dead_code)]
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

fn expression<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    skip_step: bool,
) -> Catch {
    devnote!(this it "expression");
    this.parse_precedence(mc, f, it, Precedence::Assignment, skip_step)?;

    while let Token::Comma = this.peek(it)? {
        add!(this);
        devout!("{}", "COMMAS".on_red());
        this.eat(it);
        devout!(
            "===================exp count {}",
            this.get_expression_count()
        );
        this.parse_precedence(mc, f, it, Precedence::Assignment, false)?;
    }

    Ok(())
}

/// Walk through expression precedence but stop at commas, used by arguments, and table building
fn expression_single<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    skip_step: bool,
) -> Catch {
    devnote!(this it "expression_single");
    this.parse_precedence(mc, f, it, Precedence::Assignment, skip_step)?;
    Ok(())
}

#[allow(dead_code)]
fn next_expression<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "next_expression");
    this.eat(it);
    expression(this, mc, f, it, false)?;
    Ok(())
}

fn expression_statement<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "expression_statement");
    devout!(
        "{} {}",
        "At (expression statement start)".on_cyan(),
        this.get_expression_count()
    );
    // let i = it.peek().unwrap();
    // let i2 = (*i).clone()?;
    // let i3 = i2.0.clone();
    // println!("we start at {}", i3);

    expression(this, mc, f, it, false)?;

    // Mark that the last statement was an expression for implicit returns
    this.last_was_expression = true;

    if !this.flip_override_pop() {
        // For implicit returns, we might want to keep the value(s) on the stack
        // if this is the last statement in a function, but we can't know that here
        // The function compilation will handle this by checking last_was_expression

        // If we have multiple expressions and implicit returns are enabled,
        // we might want to keep them all on the stack for the function to return
        if this.language_flags.implicit_returns && this.get_expression_count() > 1 {
            // Don't pop - keep all values for potential multiple return
        } else {
            // Pop the single expression value as usual
            this.emit_at(f, OpCode::POP);
        }
    }

    this.local_declare_mode = false;
    devnote!(this it "expression_statement end");
    Ok(())
}

fn function_expression<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "function_expression");
    // current_location was set to the `function` keyword's position by the
    // parse_precedence store() call that dispatched us here.
    let start_line = this.current_location.0;
    // build_function(this, mc, f, it, ident_clone, global_ident, false)?;
    build_function(this, mc, f, it, "".to_owned(), false, false, start_line)
}

/// Compile an arrow function `params -> body`. `params` are the parameter names
/// already collected by the caller; the `->` is the current peek and is consumed
/// here. The body is either a single expression or a `do … end` block, and is
/// ALWAYS implicitly returned (regardless of the `implicit-return` flag). Emits a
/// `CLOSURE` (+ upvalue registrations) into the enclosing function `f`.
#[cfg(feature = "arrow")]
fn build_arrow_function<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    params: Vec<String>,
    start_line: usize,
) -> Catch {
    devnote!(this it "build_arrow_function");
    expect_token!(this it ArrowFunction); // '->'
    let mut f2 = FunctionObject::new(Some("".to_owned()), false);
    f2.start_line = start_line;
    let fr2 = &mut f2;
    begin_scope(this);
    begin_functional_scope(this);
    let arity = params.len() as u8;
    for p in params {
        add_local(this, p)?;
    }
    fr2.arity = arity;
    // A normal function body opens with a POP of the (parser-absorbed) closing
    // `)` token, which the VM's call convention relies on for frame alignment.
    // An arrow has no `)` to absorb, so emit the equivalent leading POP here.
    this.emit_at(fr2, OpCode::POP);

    if let Token::Do = this.peek(it)? {
        // multi-statement body: `do … end`, last expression implicitly returned
        this.eat(it); // 'do'
        block(this, mc, fr2, it)?; // parses statements until `end`, eats `end`
        fr2.end_line = this.last_end_line;
        if let &OpCode::RETURN(_) = fr2.chunk.code.last().unwrap() {
            // an explicit `return` already closed the body
        } else {
            this.drop_last_if(fr2, &OpCode::POP);
            // arrows always implicit-return the last expression
            if !this.last_was_expression {
                this.emit_at(fr2, OpCode::NIL);
            }
            this.emit_at(fr2, OpCode::RETURN(this.get_expression_count()));
            this.set_expression_count(0);
        }
    } else {
        // single-expression body, implicitly returned. Use expression_single so a
        // following comma ends the arrow (e.g. in `f(x -> x*10, 5)` the `, 5` is
        // f's next argument, not part of the arrow body). Multi-value returns need
        // a `do … end` body.
        this.set_expression_count(1);
        expression_single(this, mc, fr2, it, false)?;
        this.emit_at(fr2, OpCode::RETURN(this.get_expression_count()));
        this.set_expression_count(0);
    }

    end_scope(this, fr2, true);
    let state = end_functional_scope(this);
    f2.upvalue_count = state.up_values.len() as u8;
    f2.is_variadic = state.vararg > 0;
    f2.varidic_index = if state.vararg > 0 { state.vararg - 1 } else { 0 };

    let func_value = Value::Function(Gc::new(mc, f2));
    let constant = f.chunk.write_constant(func_value) as u8;
    this.emit_at(f, OpCode::CLOSURE { constant });
    for val in state.up_values.iter() {
        this.emit_at(
            f,
            OpCode::REGISTER_UPVALUE {
                index: val.ident,
                neighboring: val.neighboring,
            },
        );
    }
    Ok(())
}

fn variable<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    can_assign: bool,
) -> Catch {
    devnote!(this it "variable");
    // Single-param arrow `x -> body`: the receiver ident is followed by `->`.
    #[cfg(feature = "arrow")]
    if this.language_flags.arrow_functions && matches!(this.peek(it)?, Token::ArrowFunction) {
        if let Token::Identifier(name) = this.copy_store()? {
            let line = this.current_location.0;
            return build_arrow_function(this, mc, f, it, vec![name], line);
        }
    }
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
    match this.peek(it)? {
        Token::StringLiteral(_) => call_string(this, mc, f, it, can_assign),
        Token::OpenBrace => call_table(this, mc, f, it, can_assign),
        _ => named_variable(this, mc, f, it, can_assign),
    }
}

/// This is the second concept of vararg, the usage of, not the param.
fn vararg_variable(
    this: &mut Compiler,
    _mc: &Mutation,
    f: FnRef,
    it: &mut Peekable<Lexer>,
    can_assign: bool,
) -> Catch {
    devout!("{}", "we hit here".green());
    devnote!(this it "vararg_variable");

    if can_assign {
        if let Token::Assign = this.peek(it)? {
            devout!("{}", "we error".red());
            return Err(this.error_at(SiltError::InvalidVarArgAssignment));
        }
    }

    // let mut found_vararg = false;
    // for local in this.locals.iter().rev() {
    //     if let Some(ref ident) = local.ident {
    //         if ident == "..." {
    //             found_vararg = true;
    //             break;
    //         }
    //     }
    //     // Stop searching if we hit a different functional depth
    //     if local.functional_depth < this.functional_depth {
    //         break;
    //     }
    // }
    let vararg = this.get_vararg();

    if this.functional_depth > 0 && vararg == 0 {
        devout!("{} {}", "we error".on_red(), vararg);
        return Err(this.error_at(SiltError::InvalidVarArgUsage));
    }

    // let _index = if vararg > 0 { vararg - 1 } else { 0 };
    let count = this.expected_multi;
    let is_arg = this.is_arg_mode();
    if is_arg {
        this.set_trailing_vararg(true);
    }

    this.emit_at(f, OpCode::VARARG { is_arg, count });
    add_local_placeholder(this)?;

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
            // println!("============== we're in {}", ident);
            let ident = this.identifer_constant(f, ident);
            // add_upvalue(this, ident, this.scope_depth);
            (
                OpCode::SET_GLOBAL { constant: ident },
                OpCode::GET_GLOBAL { constant: ident },
            )
        }
    }
}

fn print_var_stack(_v: &[Option<(OpCode, OpCode)>]) {
    #[cfg(feature = "dev-out")]
    {
        println!(":::::::::::::::::::::::::::::::::::::");
        print!("var stack -> ");
        for o in _v.iter() {
            if let Some(v) = o {
                print!("({},{})", v.0, v.1)
            } else {
                print!("( -, - )")
            }
        }
        println!();
        println!(":::::::::::::::::::::::::::::::::::::");
    }
}

/// Parse a `: Type` annotation (typing Phase 1). Assumes the upcoming token is
/// `:`; consumes it and a single type name, returning the parsed `Type`. Union,
/// optional (`T?`), function, and table-shape syntax are deferred to later
/// phases. Compile-time only — nothing is emitted.
#[cfg(feature = "typing")]
fn parse_type_annotation(
    this: &mut Compiler,
    it: &mut Peekable<Lexer>,
) -> Result<crate::types::Type, ErrorTuple> {
    devnote!(this it "parse_type_annotation");
    this.eat(it); // ':'
    let (res, _) = this.pop(it);
    Ok(match res? {
        Token::Identifier(name) => crate::types::Type::from_name(&name),
        Token::Nil => crate::types::Type::Nil,
        Token::Function => crate::types::Type::Function,
        other => return Err(this.error_at(SiltError::InvalidTokenPlacement(other))),
    })
}

/// Map a compound-assignment token to the binary opcode it applies.
/// `x += e` desugars to `x = x <op> e`.
#[cfg(feature = "compound-assignment")]
fn compound_op(token: &Token) -> Option<OpCode> {
    Some(match token {
        Token::AddAssign => OpCode::ADD,
        Token::SubAssign => OpCode::SUB,
        Token::MultiplyAssign => OpCode::MULTIPLY,
        Token::DivideAssign => OpCode::DIVIDE,
        Token::ModulusAssign => OpCode::MODULUS,
        Token::PowerAssign => OpCode::POWER,
        Token::FloorDivideAssign => OpCode::FLOOR_DIVIDE,
        Token::ConcatAssign => OpCode::CONCAT,
        _ => return None,
    })
}

/// `x <op>= e` for a simple variable (local / upvalue / global). The variable's
/// (setter, getter) pair was just gathered onto `var_stack`. We emit the getter
/// to push the current value, evaluate the RHS, apply `op`, then store back.
#[cfg(feature = "compound-assignment")]
fn compound_assign_var<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    op: OpCode,
) -> Catch {
    devnote!(this it "compound_assign_var");
    // Compound assignment never has multiple targets (`a, b += 1` is invalid).
    let pair = match this.var_stack.len() {
        1 => this.var_stack.pop().flatten(),
        _ => None,
    };
    let (setter, getter) = match pair {
        Some(p) => p,
        None => {
            let tok = this.peek(it)?.clone();
            return Err(this.error_at(SiltError::InvalidAssignment(tok)));
        }
    };
    this.eat(it); // the compound operator
    this.emit_at(f, getter); // current value of the target
    this.set_can_multivar_set(false);
    expression_single(this, mc, f, it, false)?; // right-hand side
    this.set_can_multivar_set(true);
    this.emit_at(f, op); // current <op> rhs
    this.emit_at(f, setter); // store result (leaves a copy on the stack)
    this.emit_at(f, OpCode::POP); // drop that copy
    this.override_pop(); // statement pop already accounted for
    Ok(())
}

fn named_variable<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
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
            add_local(this, ident)?;
            this.override_pop();

            this.local_declare_mode = true;
            // this.eat(it);
            // expression(this, f, it, false)?;
        } else {
            unreachable!()
        }
        return Ok(());
    };

    // ident getter/setter gather for 1 variable, then continue on our while loop to check for
    // more. This should usually only hit for multi var assignment

    let ops = if let Token::Identifier(ident) = t {
        // devout!("assigning to identifier: {}", ident);
        if this.local_declare_mode {
            add_local(this, ident.clone())?;
            // this.eat(it);
        }
        resolve_etters(this, f, it, ident)
    } else {
        unreachable!()
    };

    // Typed local declaration `local x: T = …` — intercept the annotation before
    // the colon reaches the method-call path. Compile-time only.
    #[cfg(feature = "typing")]
    if this.local_declare_mode && matches!(this.peek(it)?, Token::Colon) {
        let ty = parse_type_annotation(this, it)?;
        if let Some(local) = this.locals.last_mut() {
            local.ty = ty;
        }
    }

    // println!("and then it's {} {}", ops.0, this.can_multivar_set);
    this.var_stack.push(if !this.local_declare_mode {
        Some(ops)
    } else {
        None
    });
    if this.can_multivar_set {
        // These commas are intended for multivar setting aka what to assign to. x,y= ...
        // Check for additional variables in multi-variable context
        while let Token::Comma = this.peek(it)? {
            devnote!(this it "-> multi_var comma walk");
            add!(this);
            this.eat(it);
            if let Token::Identifier(_) = this.peek(it)? {
                let t = this.pop(it);
                this.current_location = t.1;

                let ops = if let Token::Identifier(ident) = t.0? {
                    if this.local_declare_mode {
                        add_local(this, ident.clone())?;
                        // this.eat(it);
                    }
                    resolve_etters(this, f, it, ident)
                } else {
                    unreachable!()
                };
                // typed multi-var: `local a: T, b: U = …`
                #[cfg(feature = "typing")]
                if this.local_declare_mode && matches!(this.peek(it)?, Token::Colon) {
                    let ty = parse_type_annotation(this, it)?;
                    if let Some(local) = this.locals.last_mut() {
                        local.ty = ty;
                    }
                }
                this.var_stack.push(if !this.local_declare_mode {
                    Some(ops)
                } else {
                    None
                });
            } else {
                // We encountered a non-identifier after comma
                // This means we have a mixed expression like "a, 5" or "a, func()"
                // For assignment, this is invalid. For retrieval, we need to handle it differently.
                let t = this.peek(it)?;

                if can_assign && matches!(t, Token::Assign) {
                    // This would be invalid assignment like "a, 5 = ..."
                    return Err(this.error_at(SiltError::InvalidAssignment(t.clone())));
                }
                // we at least know multivar setting has ended
                println!("ended multi");
                this.set_can_multivar_set(false);

                // For retrieval context, we need to drain the getters we've collected so far
                // and then continue parsing as a regular expression
                // this.return_count = this.var_stack.len() as u8;
                println!("multivar drain 1");
                this.drain_getters(f);

                // Now parse the remaining expression starting from current position
                // We need to handle this as part of a larger comma-separated expression
                // this.return_count += 1;
                this.parse_precedence(mc, f, it, Precedence::Assignment, false)?;

                return Ok(());
            }
        }
        print_var_stack(&this.var_stack);
    }
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
            // TODO is this the best fix for local passing?
            this.local_declare_mode=false;
                this.eat(it);
                let assign_need = this.var_stack.len() as isize;
                this.expected_multi = assign_need as u8;
                this.set_expression_count(1);
                // if assign_need > 1 {
                //     this.emit_at(f, OpCode::NEED(assign_need as u8));
                // }
                // println!("=============== pre setters {}", this.peek(it)?);
                std::mem::swap(&mut this.var_stack, &mut this.var_set_stack);
                // println!("set stack is {}", this.var_set_stack.len());
                this.override_pop();
                this.set_can_multivar_set(false);
                expression(this, mc, f, it, false)?;
                this.set_can_multivar_set(true);
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
                let remainder = assign_need - this.get_expression_count() as isize;
                match remainder.cmp(&0) {
                    Ordering::Greater => {
                        // we have room so spread the last if possible
                        // let offset = this.current_index - 1;
                        match f.chunk.read_last_code() {
                            OpCode::CALL(u, _, v) => {
                                // the remainder is how much MORE we would need, at least 1 is
                                // already assumed so we add 1+remainder. Preserve the variadic
                                // flag already resolved by call()/arguments().
                                // devout!("{} {}", "modify call to ".red(), remainder + 1);
                                f.chunk.patch_last(OpCode::CALL(*u, (remainder + 1) as u8, *v));
                            }
                            // we have exception for vararg because they set their own stack lengths and dont need nil padding
                            OpCode::VARARG {
                                is_arg: _,
                                count: _,
                            } => {}
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

                // println!("multivar drain 2");
                this.drain_setters(f);
            } else {
                // this.return_count = this.var_stack.len() as u8;
                this.drain_getters(f);
            }
        }
        #[cfg(feature = "compound-assignment")]
        ct @ (Token::AddAssign
        | Token::SubAssign
        | Token::MultiplyAssign
        | Token::DivideAssign
        | Token::ModulusAssign
        | Token::PowerAssign
        | Token::FloorDivideAssign
        | Token::ConcatAssign) => {
            // `x <op>= e` on a simple variable.
            if !can_assign || !this.language_flags.compound_assignment {
                let tok = ct.clone();
                return Err(this.error_at(SiltError::InvalidAssignment(tok)));
            }
            let op = compound_op(ct).unwrap();
            compound_assign_var(this, mc, f, it, op)?;
        }
        Token::OpenBracket | Token::Dot => {
            // println!("drain 4");
            this.drain_getters(f); // TODO we should probably error if this is higher then 1
            let count = table_indexer(this, mc, f, it)? as u8;
            match this.peek(it)? {
                Token::Assign => {
                    this.eat(it);
                    expression(this, mc, f, it, false)?;
                    this.emit_at(f, OpCode::TABLE_SET { depth: count });
                    // override statement end pop because instruction takes care of it
                    this.override_pop();
                }
                #[cfg(feature = "compound-assignment")]
                ct @ (Token::AddAssign
                | Token::SubAssign
                | Token::MultiplyAssign
                | Token::DivideAssign
                | Token::ModulusAssign
                | Token::PowerAssign
                | Token::FloorDivideAssign
                | Token::ConcatAssign) => {
                    // `t.f <op>= e` / `t[k] <op>= e`. Duplicate the receiver+keys
                    // (DUP_N) so the same operands feed a TABLE_GET (read current)
                    // and a TABLE_SET (store result) — no re-evaluation of `t`/`k`.
                    if !can_assign || !this.language_flags.compound_assignment {
                        let tok = ct.clone();
                        return Err(this.error_at(SiltError::InvalidAssignment(tok)));
                    }
                    let op = compound_op(ct).unwrap();
                    this.eat(it);
                    this.emit_at(f, OpCode::DUP_N(count + 1));
                    this.emit_at(f, OpCode::TABLE_GET { depth: count });
                    this.set_can_multivar_set(false);
                    expression_single(this, mc, f, it, false)?;
                    this.set_can_multivar_set(true);
                    this.emit_at(f, op);
                    this.emit_at(f, OpCode::TABLE_SET { depth: count });
                    this.override_pop();
                }
                Token::Colon => {
                    // method call on a chained receiver, e.g. `a.b:m()`. Resolve
                    // the chain to leave the receiver on the stack, then METHOD_GET.
                    this.emit_at(f, OpCode::TABLE_GET { depth: count });
                    emit_method_get(this, f, it)?;
                }
                _ => {
                    this.emit_at(f, OpCode::TABLE_GET { depth: count });
                    // add!(this);
                }
            }
        }
        Token::Colon => {
            // method call on a bare variable receiver, e.g. `t:m()`.
            this.drain_getters(f);
            emit_method_get(this, f, it)?;
        }
        _ => {
            // this.return_count = this.var_stack.len() as u8;
            // devnote!(this it "drain 5");

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

fn grouping<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "-> grouping");
    let start = this.current_location;
    // A `(` that opens with an identifier may be an arrow parameter list
    // (`(a, b) -> …`, `(a) -> …`) rather than a grouped expression. Disambiguate
    // there; everything else is an ordinary grouping.
    #[cfg(feature = "arrow")]
    if this.language_flags.arrow_functions {
        match this.peek(it)? {
            // `()` is only valid as a zero-parameter arrow `() -> body`
            Token::CloseParen => {
                this.eat(it); // ')'
                return build_arrow_function(this, mc, f, it, vec![], start.0);
            }
            Token::Identifier(_) => return grouping_or_arrow(this, mc, f, it, start),
            _ => {}
        }
    }
    expression(this, mc, f, it, false)?;
    // Consume the closing `)`. Without this the `)` is left at the cursor; since
    // it has no infix rule the enclosing precedence loop halts and any operator
    // after the group (e.g. the `* 3` in `(1+2)*3`) is silently dropped.
    expect_token!(
        this,
        it,
        CloseParen,
        this.error_at(SiltError::UnterminatedParenthesis(start.0, start.1))
    );
    Ok(())
}

/// Entered from `grouping` when a `(` is immediately followed by an identifier.
/// Resolves the ambiguity between an arrow parameter list and an ordinary
/// parenthesized expression:
///   `(a, b) -> …` / `(a) -> …` → arrow function
///   `(a)`                      → grouped variable
///   `(a + b)` / `(x -> …)`     → ordinary grouped expression
#[cfg(feature = "arrow")]
fn grouping_or_arrow<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    start: TokenCell,
) -> Catch {
    this.store(it); // current = first ident (consumed)
    let first = match this.copy_store()? {
        Token::Identifier(n) => n,
        _ => unreachable!("grouping_or_arrow entered on a non-identifier"),
    };
    match this.peek(it)? {
        Token::Comma => {
            // `(a, b, …) -> …` — a multi-parameter list (only valid as arrow params)
            let mut params = vec![first];
            while matches!(this.peek(it)?, Token::Comma) {
                this.eat(it); // ','
                let (res, _) = this.pop(it);
                match res? {
                    Token::Identifier(n) => params.push(n),
                    other => return Err(this.error_at(SiltError::InvalidTokenPlacement(other))),
                }
            }
            expect_token!(this it CloseParen);
            build_arrow_function(this, mc, f, it, params, start.0)
        }
        Token::CloseParen => {
            this.eat(it); // ')'
            if matches!(this.peek(it)?, Token::ArrowFunction) {
                // `(a) -> …` — single parenthesized parameter
                build_arrow_function(this, mc, f, it, vec![first], start.0)
            } else {
                // `(a)` — ordinary parenthesized variable; emit its getter
                let (_set, getter) = resolve_etters(this, f, it, first);
                this.emit_at(f, getter);
                Ok(())
            }
        }
        _ => {
            // ordinary grouped expression starting with an identifier, e.g.
            // `(a + b)` or `(x -> x + 1)`. Resume parsing from the stored ident.
            this.parse_precedence(mc, f, it, Precedence::Assignment, true)?;
            expect_token!(
                this,
                it,
                CloseParen,
                this.error_at(SiltError::UnterminatedParenthesis(start.0, start.1))
            );
            Ok(())
        }
    }
}

fn tabulate<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "-> tabulate");
    this.emit_at(f, OpCode::NEW_TABLE);
    // not immediately closed
    if !matches!(this.peek(it)?, &Token::CloseBrace) {
        this.set_arg_mode(true);
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
                        expression_single(this, mc, f, it, true)?; // we skip the store because the ip is already where it needs to be
                        false
                    }
                }
                Token::OpenBracket => {
                    this.eat(it);
                    expression_single(this, mc, f, it, false)?;
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
                    expression_single(this, mc, f, it, false)?; // normal store expression
                    false
                }
            } {
                println!("{}", "START TABLE FN".on_bright_cyan());
                expression_single(this, mc, f, it, false)?;
                println!("{}", "END TABLE FN".on_bright_cyan());
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
                a => {
                    println!("---------------------------------------here? 2 {}", a);
                    return Err(this.error_at(SiltError::TableExpectedCommaOrCloseBrace));
                }
            }
        } {
            // if args >= 255 {
            //     return Err(this.error_at(SiltError::TooManyParameters));
            // }
        }
        if count > 0 {
            this.emit_at(f, OpCode::TABLE_BUILD(count));
        }
        this.set_arg_mode(false);
    }

    println!("here? 1");
    expect_token!(
        this,
        it,
        CloseBrace,
        this.error_at(SiltError::TableExpectedCommaOrCloseBrace)
    );
    Ok(())
}

/** op unary or primary */
fn unary<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "unary");
    let t = this.copy_store()?;
    // self.expression();

    this.parse_precedence(mc, f, it, Precedence::Unary, false)?;
    match t {
        Token::Op(Operator::Sub) => this.emit_at(f, OpCode::NEGATE),
        Token::Op(Operator::Not) => this.emit_at(f, OpCode::NOT),
        Token::Op(Operator::Length) => this.emit_at(f, OpCode::LENGTH),
        Token::Op(Operator::Tilde) => this.emit_at(f, OpCode::BIT_NOT),
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
fn table_indexer<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Result<usize, ErrorTuple> {
    let mut count = 0;
    while match this.peek(it)? {
        Token::OpenBracket => {
            this.eat(it);
            expression(this, mc, f, it, false)?;
            expect_token!(
                this,
                it,
                CloseBracket,
                this.error_at(SiltError::UnterminatedBracket(0, 0))
            );
            true
        }
        Token::Dot => {
            single_table_index(this, f, it)?;
            true
        }
        _ => false,
    } {
        count += 1;
    }
    Ok(count)
}

fn single_table_index<'c>(
    this: &mut Compiler,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Result<(), ErrorTuple> {
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
    Ok(())
}

/// Consume `:method` after a receiver that is already on the stack and emit
/// `METHOD_GET`, leaving `[method, receiver]`. Caller sets `self_arg` so the
/// following call counts the receiver as the implicit first argument. Works for
/// any receiver expression (`t:m()`, `a.b:m()`, `f():m()`).
fn emit_method_get<'c>(
    this: &mut Compiler,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    this.eat(it); // ':'
    let (res, loc) = this.pop(it);
    this.current_location = loc;
    let name = match res? {
        Token::Identifier(ident) => ident,
        _ => return Err(this.error_at(SiltError::ExpectedFieldIdentifier)),
    };
    let constant = this.identifer_constant(f, name);
    this.emit_at(f, OpCode::METHOD_GET { constant });
    this.self_arg = true;
    Ok(())
}

/// Infix `:method` after any expression whose value is already on the stack —
/// e.g. `("hi"):upper()`, `f():m()`. The Pratt loop has already consumed the
/// `:` (via `store`), so unlike `emit_method_get` we read the method name
/// directly. Identifier/table receivers are handled earlier in `named_variable`,
/// so this only fires for grouped / call-result receivers.
fn method_infix<'c>(
    this: &mut Compiler,
    _mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "method_infix");
    let (res, loc) = this.pop(it);
    this.current_location = loc;
    let name = match res? {
        Token::Identifier(ident) => ident,
        _ => return Err(this.error_at(SiltError::ExpectedFieldIdentifier)),
    };
    let constant = this.identifer_constant(f, name);
    this.emit_at(f, OpCode::METHOD_GET { constant });
    this.self_arg = true;
    Ok(())
}

fn binary<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "binary");
    let t = this.copy_store()?;
    let l = this.current_location;
    let rule = Compiler::get_rule(&t);
    this.parse_precedence(mc, f, it, rule.precedence.next(), false)?;
    if let Token::Op(op) = t {
        match op {
            Operator::Add => this.emit(f, OpCode::ADD, l),
            Operator::Sub => this.emit(f, OpCode::SUB, l),
            Operator::Multiply => this.emit(f, OpCode::MULTIPLY, l),
            Operator::Divide => this.emit(f, OpCode::DIVIDE, l),
            Operator::Modulus => this.emit(f, OpCode::MODULUS, l),
            Operator::FloorDivide => this.emit(f, OpCode::FLOOR_DIVIDE, l),
            Operator::BitAnd => this.emit(f, OpCode::BIT_AND, l),
            Operator::BitOr => this.emit(f, OpCode::BIT_OR, l),
            Operator::Tilde => this.emit(f, OpCode::BIT_XOR, l),
            Operator::ShiftLeft => this.emit(f, OpCode::SHIFT_LEFT, l),
            Operator::ShiftRight => this.emit(f, OpCode::SHIFT_RIGHT, l),

            Operator::Concat => this.emit(f, OpCode::CONCAT, l),

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

/// Right-associative `^`. Parses the RHS at its OWN precedence (not `.next()`)
/// so `2^2^3` groups as `2^(2^3)`. Because Exponent sits above Unary, `-2^2`
/// already parses as `-(2^2)` via the unary operand parse.
fn exponent<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "exponent");
    let l = this.current_location;
    this.parse_precedence(mc, f, it, Precedence::Exponent, false)?;
    this.emit(f, OpCode::POWER, l);
    Ok(())
}

fn concat<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "concat_binary");
    let t = this.copy_store()?;
    let l = this.current_location;
    let rule = Compiler::get_rule(&t);
    this.parse_precedence(mc, f, it, rule.precedence.next(), false)?;

    if let Token::Op(op) = t {
        match op {
            Operator::Concat => this.emit(f, OpCode::CONCAT, l),
            _ => todo!(),
        }
    }
    Ok(())
}

fn and<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this it "and");
    let index = this.emit_index(f, OpCode::GOTO_IF_FALSE(0));
    this.emit_at(f, OpCode::POP);
    this.parse_precedence(mc, f, it, Precedence::And, false)?;
    this.patch(f, index)?;
    Ok(())
}

fn or<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
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
    this.parse_precedence(mc, f, it, Precedence::Or, false)?;
    this.patch(f, index)?;
    Ok(())
}

fn integer<'c>(
    this: &mut Compiler,
    _mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    _it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this _it "integer");
    let t = this.copy_store()?;
    let value = if let Token::Integer(i) = t {
        Value::Integer(i)
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn number<'c>(
    this: &mut Compiler,
    _mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    _it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this _it "number");
    let t = this.copy_store()?;
    let value = if let Token::Number(n) = t {
        Value::Number(n)
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn string<'c>(
    this: &mut Compiler,
    _mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    _it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this _it "string");
    let t = this.copy_store()?;
    let value = if let Token::StringLiteral(s) = t {
        Value::String(s.into_string())
    } else {
        unreachable!()
    };
    this.constant_at(f, value);
    Ok(())
}

fn literal<'c>(
    this: &mut Compiler,
    _mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    _it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(this _it "literal");
    let t = this.copy_store()?;
    match t {
        Token::Nil => this.emit_at(f, OpCode::NIL),
        Token::True => this.emit_at(f, OpCode::TRUE),
        Token::False => this.emit_at(f, OpCode::FALSE),
        _ => unreachable!(),
    }
    Ok(())
}

fn call<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
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
    let arg_count = arguments(this, mc, f, it, start)?;
    devout!("{} {}", "ARG COUNT".on_cyan(), arg_count);
    // If the final argument was `...`, the call spreads the enclosing function's
    // variadic overflow, so the real argument count is resolved at runtime.
    let trailing = this.is_trailing_vararg();
    this.emit(f, OpCode::CALL(arg_count, 0, trailing), start);
    if trailing {
        this.set_trailing_vararg(false);
    }
    Ok(())
}

fn call_table<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    can_assign: bool,
) -> Catch {
    let start = this.current_location;

    this.set_arg_mode(true);
    this.set_can_multivar_set(false);

    this.eat(it);
    tabulate(this, mc, f, it, can_assign)?;

    this.set_arg_mode(false);
    this.set_can_multivar_set(true);
    this.emit(f, OpCode::CALL(1, 0,false), start);
    Ok(())
}

fn call_string<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    let start = this.current_location;

    this.set_arg_mode(true);
    this.set_can_multivar_set(false);

    expression_single(this, mc, f, it, false)?;

    this.set_arg_mode(false);
    this.set_can_multivar_set(true);
    this.emit(f, OpCode::CALL(1, 0,false), start);
    Ok(())
}

fn arguments<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
    start: TokenCell,
) -> Result<u8, ErrorTuple> {
    devnote!(this it "arguments");
    this.set_arg_mode(true);
    this.set_can_multivar_set(false);

    // self was pushed on the stack recently, include it and turn off
    let mut args = if this.self_arg {
        this.self_arg = false;
        1
    } else {
        0
    };
    // let mut _has_vararg = false;

    devout!("{} {}", "start with ".red(), args);
    if !matches!(this.peek(it)?, &Token::CloseParen) {
        while {
            // Check if this is a vararg expression
            if let Token::VarArg = this.peek(it)? {
                this.store(it); // consume the VarArg token
                vararg_variable(this, mc, f, it, false)?;
                args += 1;
                // _has_vararg = true;
                // VarArg must be the last argument
                if let Token::Comma = this.peek(it)? {
                    // TODO: Add SiltError::VarArgMustBeLast to error types
                    return Err(this.error_at(SiltError::ExpectedLocalIdentifier));
                    // Placeholder
                }
                false // Don't continue the loop
            } else {
                expression_single(this, mc, f, it, false)?;
                devout!("{}", "yeah ADD 1".red());
                args += 1;
                if let &Token::Comma = this.peek(it)? {
                    this.eat(it);
                    true
                } else {
                    false
                }
            }
        } {
            devout!("{} {}", "yeah done ading".red(), args);
            if args == 255 {
                return Err(this.error_at(SiltError::TooManyParameters));
            }
        }
    }
    this.set_arg_mode(false);
    this.set_can_multivar_set(true);

    expect_token!(
        this,
        it,
        CloseParen,
        this.error_at(SiltError::UnterminatedParenthesis(start.0, start.1))
    );
    devout!("arguments count: {}", args);

    // if has_vararg {
    //     Ok(255) // Special value indicating vararg call
    // } else {
    //     Ok(args)
    // }
    Ok(args)
}

fn print<'c>(
    this: &mut Compiler,
    mc: &Mutation<'c>,
    f: FnRef<'_, 'c>,
    it: &mut Peekable<Lexer>,
) -> Catch {
    devnote!(this it "print");
    this.eat(it);
    expression(this, mc, f, it, false)?;
    this.emit_at(f, OpCode::PRINT);
    Ok(())
}

pub fn void<'c>(
    _this: &mut Compiler,
    _mc: &Mutation<'c>,
    _f: FnRef<'_, 'c>,
    _it: &mut Peekable<Lexer>,
    _can_assign: bool,
) -> Catch {
    devnote!(_this _it "void");
    Ok(())
}

pub(crate) fn to_op_string(name: Option<&str>) -> Option<String> {
    name.map(|o| o.to_string())
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
