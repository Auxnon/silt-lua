//! Luau-style gradual static typing, woven into the single-pass compiler
//! (`typing` feature; compile-time only — bytecode is unchanged).
//!
//! Three pieces:
//!
//! * **Declaration pre-scan.** Before the main pass a token walk collects
//!   `type Name = T` aliases and `function name(a: T): R` signatures into the
//!   type environment, so a use before its definition (forward references,
//!   mutually-recursive functions) checks against the real signature instead of
//!   degrading to `any`. It reads declarations only and emits nothing.
//! * **Type stack.** A stack of static types mirroring the operand stack, kept
//!   up to date by [`Compiler::tc_effect`] from the one place every opcode is
//!   written (`write_code`). Operators pop operand types, check them, push the
//!   result; `SET_*`/`RETURN`/`CALL` check against the declared types. It is
//!   cleared at every statement boundary, so any bookkeeping drift is contained
//!   to one statement and can only ever cause a *missed* check, never a false
//!   one on a later line.
//! * **Narrowing.** `if x then` / `if x ~= nil then` / `if type(x) == "..." then`
//!   refine `x`'s type inside the branch.
//!
//! Gradual rules: a check fires only when both sides are concretely known and
//! provably incompatible. Without `--!strict` (Luau nonstrict) only *annotated
//! boundaries* are checked — assignments to typed locals, arguments against
//! typed parameters, returns against declared return types, fields of typed
//! table shapes — and unannotated locals stay `any`. `--!strict` adds operator
//! checks (arithmetic/concat/compare on a value that can never be a number or
//! string, indexing or calling a primitive, argument counts) and infers
//! unannotated locals from their initializers. Type errors are compile errors.

use std::collections::{HashMap, HashSet};
use std::iter::Peekable;

use crate::{
    code::{OpCode, MULTIRET},
    error::{SiltError, TokenCell},
    function::FunctionObject,
    lexer::Lexer,
    token::{Operator, Token},
    types::{assignable, FnSig, Snippetness, TableShape, Type},
    value::Value,
};

use super::{Compiler, FunctionalState};

/// Host declarations. The only checker state that survives from one compile to
/// the next (`Compiler::declare*`), since it describes the embedding, not a chunk.
#[derive(Default, Clone)]
pub(crate) struct Declarations {
    /// Static types of host-provided globals (`instr`, `emit`, …).
    pub globals: HashMap<String, Type>,
    /// Globals the host also provides inside snippet VMs, so a snippet may
    /// reference them (the standard library always qualifies).
    pub in_snippet: HashSet<String>,
}

/// Per-compile type environment and checker state.
#[derive(Default)]
pub(crate) struct TypeCheck {
    /// Host declarations (carried across compiles).
    pub declared: Declarations,
    /// `type Name = T`
    pub aliases: HashMap<String, Type>,
    /// Named function signatures (pre-scanned `function f(...)` /
    /// `local function f(...)` plus those seen in the main pass).
    pub fn_sigs: HashMap<String, FnSig>,
    /// `global x: T` annotations in the script being compiled.
    pub script_globals: HashMap<String, Type>,
    /// Static types of the temporaries currently above the locals.
    pub stack: Vec<Type>,
    /// Parallel to `stack`: where each value's expression starts in the source, so
    /// a type error can underline the whole offending expression.
    pub starts: Vec<TokenCell>,
    /// Location of the op `tc_effect` is applying.
    pub op_loc: TokenCell,
    /// Earliest start among the values the current op has popped.
    pub popped_start: Option<TokenCell>,
    /// Start of the value a check outside `tc_effect` is about (`tc_declare_locals`).
    pub error_start: Option<TokenCell>,
    /// Locals refined by an enclosing `if`, with their unrefined types (outermost
    /// first): an assignment is checked against the unrefined type.
    pub narrowed: Vec<(usize, Type)>,
    /// `--!strict`: infer unannotated locals from their initializers.
    pub strict: bool,
    /// Signature of the function body just compiled, consumed by its CLOSURE.
    pub pending_sig: Option<FnSig>,
    /// The string constant emitted by the previous instruction, if any (the key
    /// of a `t.field` access about to be emitted).
    pub last_const_str: Option<String>,
    /// The RETURN being emitted is the compiler's own fall-off-the-end return.
    pub implicit_return: bool,
}

/// A refinement applied to one local inside an `if` branch.
pub(crate) struct Narrow {
    local: usize,
    saved: Type,
    then_ty: Option<Type>,
    else_ty: Option<Type>,
}

fn arith_name(op: &OpCode) -> &'static str {
    match op {
        OpCode::ADD => "+",
        OpCode::SUB => "-",
        OpCode::MULTIPLY => "*",
        OpCode::DIVIDE => "/",
        OpCode::MODULUS => "%",
        OpCode::POWER => "^",
        OpCode::FLOOR_DIVIDE => "//",
        OpCode::BIT_AND => "&",
        OpCode::BIT_OR => "|",
        OpCode::BIT_XOR => "~",
        OpCode::SHIFT_LEFT => "<<",
        OpCode::SHIFT_RIGHT => ">>",
        OpCode::LESS => "<",
        OpCode::LESS_EQUAL => "<=",
        OpCode::GREATER => ">",
        OpCode::GREATER_EQUAL => ">=",
        _ => "?",
    }
}

/// A type that can never be a number, even through coercion or a metamethod.
fn never_number(t: &Type) -> bool {
    match t {
        // a union (or `T?`, which may be nil) fails if any member can't be a number
        Type::Union(parts) => parts.iter().any(never_number),
        Type::Optional(_) => true,
        t => matches!(
            t.widen(),
            Type::String | Type::Boolean | Type::Nil | Type::Function(_) | Type::Snippet(_)
        ),
    }
}

/// A type whose every value is a reference (a function, or a table known to
/// hold one): a snippet cannot capture it by value.
fn is_reference(t: &Type) -> bool {
    match t {
        Type::Function(_) | Type::Snippet(_) => true,
        Type::Table(Some(shape)) => {
            shape.fields.iter().any(|(_, t)| is_reference(t))
                || shape.array.as_ref().map_or(false, is_reference)
                || shape.index.as_ref().map_or(false, |(_, v)| is_reference(v))
        }
        Type::Union(parts) => !parts.is_empty() && parts.iter().all(is_reference),
        _ => false,
    }
}

impl Compiler {
    // ------------------------------------------------------------------ stack

    /// Push a value's type; it starts at the earliest of the operands it consumed and
    /// the op itself (a call at its callee, `a + b` at `a`, `#s` at the `#`).
    fn tpush(&mut self, t: Type) {
        self.tc.stack.push(t);
        let start = self.tc.popped_start.map_or(self.tc.op_loc, |p| p.min(self.tc.op_loc));
        self.tc.starts.push(start);
    }

    fn note_popped(&mut self, start: Option<TokenCell>) {
        if let Some(s) = start {
            self.tc.popped_start = Some(self.tc.popped_start.map_or(s, |p| p.min(s)));
        }
    }

    /// Pop a type; an empty stack (bookkeeping drift) reads as `any`.
    fn tpop(&mut self) -> Type {
        let start = self.tc.starts.pop();
        self.note_popped(start);
        self.tc.stack.pop().unwrap_or(Type::Any)
    }

    /// Pop and resolve aliases.
    /// Pop with aliases resolved and singletons widened (operand checks).
    fn tpop_resolved(&mut self) -> Type {
        let t = self.tpop();
        self.resolve_ty(&t).widen()
    }

    /// Pop `n` types, returned bottom-to-top.
    fn tpopn(&mut self, n: usize) -> Vec<Type> {
        let len = self.tc.stack.len();
        let starts = self.tc.starts.split_off(self.tc.starts.len().saturating_sub(n));
        self.note_popped(starts.iter().min().copied());
        if n >= len {
            let mut v = vec![Type::Any; n - len];
            v.extend(self.tc.stack.drain(..));
            v
        } else {
            self.tc.stack.split_off(len - n)
        }
    }

    /// Statement boundary: the operand stack holds only locals here.
    pub(crate) fn tc_reset_statement(&mut self) {
        self.tc.stack.clear();
        self.tc.starts.clear();
        self.tc.last_const_str = None;
    }

    /// A type error spanning the offending expression: from the start of the value(s)
    /// involved (or `loc`, whichever is earlier) to the end of the last token consumed.
    fn type_error(&mut self, msg: String, loc: TokenCell) {
        let start = self
            .tc
            .error_start
            .take()
            .or(self.tc.popped_start)
            .map_or(loc, |s| s.min(loc));
        let mut e = self.error_syntax(SiltError::TypeError(msg), start);
        if self.last_end >= start {
            e.end = Some(self.last_end);
        }
        self.push_error(e);
    }

    /// Replace alias names with their definitions (bounded, so a recursive
    /// alias simply stays `Named`, i.e. `any`).
    pub(crate) fn resolve_ty(&self, t: &Type) -> Type {
        self.resolve_depth(t, 0)
    }

    fn resolve_depth(&self, t: &Type, depth: usize) -> Type {
        if depth > 6 {
            return t.clone();
        }
        match t {
            Type::Named(n) => match self.tc.aliases.get(n) {
                Some(def) => self.resolve_depth(def, depth + 1),
                None => t.clone(),
            },
            Type::Optional(inner) => Type::Optional(Box::new(self.resolve_depth(inner, depth + 1))),
            Type::Union(parts) => Type::union(
                parts
                    .iter()
                    .map(|p| self.resolve_depth(p, depth + 1))
                    .collect(),
            ),
            Type::Table(Some(shape)) => {
                let mut s = (**shape).clone();
                for (_, ty) in s.fields.iter_mut() {
                    *ty = self.resolve_depth(ty, depth + 1);
                }
                if let Some(a) = &s.array {
                    s.array = Some(self.resolve_depth(a, depth + 1));
                }
                if let Some((k, v)) = &s.index {
                    s.index = Some((
                        self.resolve_depth(k, depth + 1),
                        self.resolve_depth(v, depth + 1),
                    ));
                }
                Type::Table(Some(Box::new(s)))
            }
            Type::Function(Some(sig)) => {
                Type::Function(Some(Box::new(self.resolve_sig(sig, depth + 1))))
            }
            Type::Snippet(Some(sig)) => {
                Type::Snippet(Some(Box::new(self.resolve_sig(sig, depth + 1))))
            }
            other => other.clone(),
        }
    }

    fn resolve_sig(&self, sig: &FnSig, depth: usize) -> FnSig {
        FnSig {
            params: sig
                .params
                .iter()
                .map(|p| self.resolve_depth(p, depth))
                .collect(),
            variadic: sig.variadic,
            returns: sig
                .returns
                .as_ref()
                .map(|r| r.iter().map(|t| self.resolve_depth(t, depth)).collect()),
            snippet: sig.snippet.clone(),
        }
    }

    /// "Type 'v' could not be converted into 't'", plus the reason when a
    /// function literal was rejected as a snippet.
    fn conversion_error(&self, target: &Type, value: &Type) -> String {
        let (target, mut value) = (self.resolve_ty(target), self.resolve_ty(value));
        // name the literal only when the slot itself wants particular literals
        if !target.has_literal() {
            value = value.widen();
        }
        let mut msg = format!("Type '{}' could not be converted into '{}'", value, target);
        if let (Type::Snippet(_), Type::Function(Some(sig))) = (&target, &value) {
            if let Snippetness::No(why) = &sig.snippet {
                msg.push_str(": ");
                msg.push_str(why);
            }
        }
        msg
    }

    fn fits(&self, target: &Type, value: &Type) -> bool {
        assignable(&self.resolve_ty(target), &self.resolve_ty(value))
    }

    // ------------------------------------------------------------- variables

    /// `locals` index of the local at operand-stack slot `slot` of the current
    /// function (the inverse of `resolve_local`'s offset arithmetic).
    fn local_index(&self, slot: u8) -> usize {
        let offset = if self.functional_depth > 0 {
            self.local_functional_offset[self.functional_depth - 1]
        } else {
            0
        };
        slot as usize + offset
    }

    fn local_ty(&self, slot: u8) -> Type {
        self.locals
            .get(self.local_index(slot))
            .map(|l| l.ty.clone())
            .unwrap_or(Type::Any)
    }

    fn upvalue_ty(&self, index: u8) -> Type {
        if self.functional_depth == 0 {
            return Type::Any;
        }
        self.functional_states[self.functional_depth - 1]
            .up_values
            .get(index as usize)
            .map(|u| u.ty.clone())
            .unwrap_or(Type::Any)
    }

    fn global_ty(&self, name: &str) -> Type {
        if let Some(t) = self.tc.script_globals.get(name) {
            return t.clone();
        }
        if let Some(t) = self.tc.declared.globals.get(name) {
            return t.clone();
        }
        if let Some(sig) = self.tc.fn_sigs.get(name) {
            return Type::Function(Some(Box::new(sig.clone())));
        }
        builtin_global(name).unwrap_or(Type::Any)
    }

    /// A global was read or written inside a function body: remember it for
    /// that function's snippet verdict.
    fn note_global(&mut self, name: &str) {
        if let Some(st) = self.functional_states.last_mut() {
            if !st.referenced_globals.iter().any(|g| g == name) {
                st.referenced_globals.push(name.to_string());
            }
        }
    }

    fn const_ty(&mut self, f: &FunctionObject, constant: u16) -> Type {
        match f.chunk.get_constant(constant) {
            Value::Integer(_) | Value::Number(_) | Value::Infinity(_) => Type::Number,
            Value::String(s) => {
                self.tc.last_const_str = Some(s.to_string());
                Type::StringLit(s.to_string())
            }
            Value::Bool(b) => Type::BoolLit(*b),
            Value::Nil => Type::Nil,
            Value::Function(_) | Value::Closure(_) | Value::NativeFunction(_) => {
                Type::Function(None)
            }
            Value::Table(_) => Type::Table(None),
            _ => Type::Any,
        }
    }

    fn const_name(f: &FunctionObject, constant: u16) -> Option<String> {
        match f.chunk.get_constant(constant) {
            Value::String(s) => Some(s.to_string()),
            _ => None,
        }
    }

    // ------------------------------------------------------------- the hook

    /// Apply an instruction's static effect to the type stack and run the
    /// checks it implies. Called from `write_code` for every emitted op.
    pub(crate) fn tc_effect(&mut self, f: &FunctionObject, op: &OpCode, loc: TokenCell) {
        use OpCode::*;
        // Only a string CONSTANT leaves its value for the *next* instruction.
        let prev_const = self.tc.last_const_str.take();
        self.tc.op_loc = loc;
        self.tc.popped_start = None;
        match op {
            CONSTANT { constant } => {
                let t = self.const_ty(f, *constant);
                self.tpush(t);
            }
            CLOSURE { .. } => {
                let sig = self.tc.pending_sig.take();
                self.tpush(Type::Function(sig.map(Box::new)));
            }
            GET_GLOBAL { constant } => {
                let t = match Self::const_name(f, *constant) {
                    Some(n) => {
                        self.note_global(&n);
                        self.global_ty(&n)
                    }
                    None => Type::Any,
                };
                self.tpush(t);
            }
            DEFINE_GLOBAL { constant } | SET_GLOBAL { constant } => {
                let v = self.tpop();
                if let Some(name) = Self::const_name(f, *constant) {
                    self.note_global(&name);
                    if let Some(declared) = self.tc.script_globals.get(&name) {
                        let target = self.resolve_ty(declared);
                        if !target.is_unknown() && !self.fits(&target, &v) {
                            let msg = self.conversion_error(&target, &v);
                            self.type_error(msg, loc);
                        }
                    } else if let (Some(sig), Type::Function(Some(vs))) =
                        (self.tc.fn_sigs.get(&name), &v)
                    {
                        let target = Type::Function(Some(Box::new(sig.clone())));
                        let value = Type::Function(Some(vs.clone()));
                        if !self.fits(&target, &value) {
                            let msg = self.conversion_error(&target, &value);
                            self.type_error(msg, loc);
                        }
                    }
                }
            }
            GET_LOCAL { index } => {
                let t = self.local_ty(*index);
                self.tpush(t);
            }
            SET_LOCAL { index } => {
                let v = self.tpop();
                let idx = self.local_index(*index);
                match self.tc.narrowed.iter().find(|(i, _)| *i == idx).map(|(_, t)| t.clone()) {
                    // `if x == nil then x = "" end`: check against what `x` was
                    // declared as, and drop the refinement the assignment voids
                    Some(declared) => {
                        self.check_assign(&declared, &v, loc);
                        if let Some(l) = self.locals.get_mut(idx) {
                            l.ty = declared;
                        }
                    }
                    None => {
                        let target = self.local_ty(*index);
                        self.check_assign(&target, &v, loc);
                    }
                }
            }
            GET_UPVALUE { index } => {
                let t = self.upvalue_ty(*index);
                self.tpush(t);
            }
            SET_UPVALUE { index } => {
                let v = self.tpop();
                let target = self.upvalue_ty(*index);
                self.check_assign(&target, &v, loc);
            }
            DEFINE_LOCAL { .. } => {}
            // `a and b` / `a or b`: the left operand is the join value if the
            // jump is taken, the right one otherwise — unknown here.
            GOTO_IF_FALSE(_) | GOTO_IF_TRUE(_) => {
                if let Some(top) = self.tc.stack.last_mut() {
                    *top = Type::Any;
                }
            }
            POP_AND_GOTO_IF_FALSE(_) | POP | PRINT => {
                self.tpop();
            }
            POPS(n) | CLOSE_UPVALUES(n) => {
                self.tpopn(*n as usize);
            }
            FOR_NUMERIC(_) => self.tpush(Type::Number),
            FORLOOP(..)
            | FORWARD(_)
            | REWIND(_)
            | INCREMENT { .. }
            | REGISTER_UPVALUE { .. }
            | NEED(_)
            | LITERAL { .. }
            | META(_)
            | TABLE_GET_FROM { .. } => {}
            RETURN(n) => {
                let vals = self.tpopn(*n as usize);
                if !self.tc.implicit_return {
                    self.check_return(&vals, loc);
                }
            }
            ADD | SUB | MULTIPLY | DIVIDE | MODULUS | POWER | FLOOR_DIVIDE | BIT_AND | BIT_OR
            | BIT_XOR | SHIFT_LEFT | SHIFT_RIGHT => {
                let r = self.tpop();
                let l = self.tpop();
                let (l, r) = (self.resolve_ty(&l), self.resolve_ty(&r));
                if self.tc.strict {
                    for t in [&l, &r] {
                        if never_number(t) {
                            self.type_error(
                                format!(
                                    "Type '{}' could not be converted into 'number' (operator {})",
                                    t,
                                    arith_name(op)
                                ),
                                loc,
                            );
                        }
                    }
                }
                // a table operand may carry a metamethod with any result
                let result = if matches!(l, Type::Table(_)) || matches!(r, Type::Table(_)) {
                    Type::Any
                } else {
                    Type::Number
                };
                self.tpush(result);
            }
            NEGATE => {
                let v = self.tpop_resolved();
                if self.tc.strict && never_number(&v) {
                    self.type_error(format!("Type '{}' could not be negated", v), loc);
                }
                self.tpush(if matches!(v, Type::Table(_)) {
                    Type::Any
                } else {
                    Type::Number
                });
            }
            BIT_NOT => {
                let v = self.tpop_resolved();
                if self.tc.strict && never_number(&v) {
                    self.type_error(
                        format!(
                            "Type '{}' could not be converted into 'number' (operator ~)",
                            v
                        ),
                        loc,
                    );
                }
                self.tpush(Type::Number);
            }
            CONCAT => {
                let r = self.tpop_resolved();
                let l = self.tpop_resolved();
                for t in [&l, &r] {
                    if self.tc.strict
                        && matches!(
                            t,
                            Type::Boolean | Type::Nil | Type::Function(_) | Type::Snippet(_)
                        )
                    {
                        self.type_error(
                            format!(
                                "Type '{}' could not be converted into 'string' (operator ..)",
                                t
                            ),
                            loc,
                        );
                    }
                }
                self.tpush(Type::String);
            }
            NOT => {
                self.tpop();
                self.tpush(Type::Boolean);
            }
            EQUAL | NOT_EQUAL => {
                self.tpop();
                self.tpop();
                self.tpush(Type::Boolean);
            }
            LESS | LESS_EQUAL | GREATER | GREATER_EQUAL => {
                let r = self.tpop_resolved();
                let l = self.tpop_resolved();
                let comparable = |t: &Type| matches!(t, Type::Number | Type::String);
                if self.tc.strict && !l.is_unknown() && !r.is_unknown() {
                    let ok = (comparable(&l) && l == r)
                        || matches!(l, Type::Table(_))
                        || matches!(r, Type::Table(_));
                    if !ok {
                        self.type_error(
                            format!(
                                "Types '{}' and '{}' cannot be compared with {}",
                                l,
                                r,
                                arith_name(op)
                            ),
                            loc,
                        );
                    }
                }
                self.tpush(Type::Boolean);
            }
            LENGTH => {
                let v = self.tpop_resolved();
                if self.tc.strict
                    && matches!(
                        v,
                        Type::Number
                            | Type::Boolean
                            | Type::Nil
                            | Type::Function(_)
                            | Type::Snippet(_)
                    )
                {
                    self.type_error(format!("Cannot take the length of '{}'", v), loc);
                }
                self.tpush(Type::Number);
            }
            NIL => self.tpush(Type::Nil),
            NILS(n) => {
                for _ in 0..*n {
                    self.tpush(Type::Nil);
                }
            }
            TRUE => self.tpush(Type::BoolLit(true)),
            FALSE => self.tpush(Type::BoolLit(false)),
            NEW_TABLE => self.tpush(Type::Table(None)),
            TABLE_INSERT { .. } => {
                self.tpop();
                self.tpop();
            }
            TABLE_BUILD { count, variadic } => {
                let n = if *variadic {
                    (*count as usize).saturating_sub(1)
                } else {
                    *count as usize
                };
                self.tpopn(n);
            }
            TABLE_GET { depth } => {
                let keys = self.tpopn(*depth as usize);
                let recv = self.tpop();
                let field = if *depth == 1 {
                    prev_const.clone()
                } else {
                    None
                };
                let key_ty = keys.last().cloned().unwrap_or(Type::Any);
                let t = self.index_result(&recv, field.as_deref(), &key_ty, loc);
                self.tpush(t);
            }
            TABLE_GET_BY_CONSTANT { constant } => {
                let recv = self.tpop();
                let field = Self::const_name(f, *constant);
                let t = self.index_result(&recv, field.as_deref(), &Type::String, loc);
                self.tpush(t);
            }
            METHOD_GET { constant } => {
                let recv = self.tpop();
                let field = Self::const_name(f, *constant);
                let method = match self.resolve_ty(&recv) {
                    Type::Table(Some(shape)) => field
                        .as_deref()
                        .and_then(|n| shape.field(n).cloned())
                        .unwrap_or(Type::Any),
                    Type::Number | Type::Boolean | Type::Nil => {
                        if self.tc.strict {
                            self.type_error(
                                format!("Cannot index {}", self.resolve_ty(&recv)),
                                loc,
                            );
                        }
                        Type::Any
                    }
                    _ => Type::Any,
                };
                self.tpush(method);
                self.tpush(recv);
            }
            TABLE_SET { depth } => {
                self.tpop(); // value
                self.tpopn(*depth as usize); // keys
                let recv = self.tpop_resolved();
                if self.tc.strict
                    && matches!(
                        recv,
                        Type::Number
                            | Type::Boolean
                            | Type::Nil
                            | Type::Function(_)
                            | Type::Snippet(_)
                    )
                {
                    self.type_error(format!("Cannot index {}", recv), loc);
                }
            }
            FOR_GENERIC { count, .. } => {
                for _ in 0..*count {
                    self.tpush(Type::Any);
                }
            }
            VARARG { is_arg, count } => {
                if !*is_arg {
                    for _ in 0..*count {
                        self.tpush(Type::Any);
                    }
                }
            }
            DUP_N(n) => {
                let len = self.tc.stack.len();
                let from = len.saturating_sub(*n as usize);
                let copies: Vec<Type> = self.tc.stack[from..].to_vec();
                self.tc.stack.extend(copies);
                let sfrom = self.tc.starts.len().saturating_sub(*n as usize);
                let scopies: Vec<TokenCell> = self.tc.starts[sfrom..].to_vec();
                self.tc.starts.extend(scopies);
            }
            CALL(arity, multi, variadic) => {
                let want = if *multi == MULTIRET {
                    1
                } else {
                    (*multi).max(1) as usize
                };
                self.check_call(*arity as usize, *variadic, want, loc);
            }
            TAILCALL(arity, variadic) => {
                self.check_call(*arity as usize, *variadic, 1, loc);
            }
        }
    }

    fn check_assign(&mut self, target: &Type, value: &Type, loc: TokenCell) {
        if !self.fits(target, value) {
            let msg = self.conversion_error(target, value);
            self.type_error(msg, loc);
        }
    }

    fn check_return(&mut self, vals: &[Type], loc: TokenCell) {
        if self.functional_depth == 0 {
            return;
        }
        let declared = match &self.functional_states[self.functional_depth - 1].ret_types {
            Some(r) => r.clone(),
            None => return,
        };
        if vals.len() > declared.len() {
            self.type_error(
                format!(
                    "Function returns {} value{} but {} {} declared",
                    vals.len(),
                    if vals.len() == 1 { "" } else { "s" },
                    declared.len(),
                    if declared.len() == 1 { "is" } else { "are" }
                ),
                loc,
            );
        }
        for (i, want) in declared.iter().enumerate() {
            let got = vals.get(i).cloned().unwrap_or(Type::Nil);
            if !self.fits(want, &got) {
                let msg = format!(
                    "{} (return value {})",
                    self.conversion_error(want, &got),
                    i + 1
                );
                self.type_error(msg, loc);
            }
        }
    }

    /// Pop callee + args, check them against the callee's signature when known,
    /// push `want` result types.
    fn check_call(&mut self, arity: usize, variadic: bool, want: usize, loc: TokenCell) {
        let args = self.tpopn(arity);
        let callee = self.tpop_resolved();
        let mut results: Option<Vec<Type>> = None;
        match &callee {
            Type::Function(Some(sig)) | Type::Snippet(Some(sig)) => {
                // a spread `...` / multiret argument makes the count run-time
                if !variadic {
                    if self.tc.strict && args.len() > sig.params.len() && !sig.variadic {
                        self.type_error(
                            format!(
                                "Argument count mismatch. Function expects {} argument{}, but {} {} specified",
                                sig.params.len(),
                                if sig.params.len() == 1 { "" } else { "s" },
                                args.len(),
                                if args.len() == 1 { "is" } else { "are" }
                            ),
                            loc,
                        );
                    }
                    for (i, p) in sig.params.iter().enumerate() {
                        // a missing trailing argument is nil
                        let a = args.get(i).cloned().unwrap_or(Type::Nil);
                        if !self.fits(p, &a) {
                            let msg =
                                format!("{} (argument {})", self.conversion_error(p, &a), i + 1);
                            self.type_error(msg, loc);
                        }
                    }
                }
                results = sig.returns.clone();
            }
            Type::Number | Type::String | Type::Boolean | Type::Nil => {
                if self.tc.strict {
                    self.type_error(format!("Cannot call non-function {}", callee), loc);
                }
            }
            _ => {}
        }
        for i in 0..want {
            let t = match &results {
                Some(r) => r.get(i).cloned().unwrap_or(Type::Nil),
                None => Type::Any,
            };
            self.tpush(t);
        }
    }

    fn index_result(
        &mut self,
        recv: &Type,
        field: Option<&str>,
        key: &Type,
        loc: TokenCell,
    ) -> Type {
        match self.resolve_ty(recv) {
            Type::Number | Type::Boolean | Type::Nil | Type::Function(_) | Type::Snippet(_) => {
                if self.tc.strict {
                    self.type_error(format!("Cannot index {}", self.resolve_ty(recv)), loc);
                }
                Type::Any
            }
            Type::Table(Some(shape)) => {
                if let Some(name) = field {
                    if let Some(t) = shape.field(name) {
                        return t.clone();
                    }
                    if let Some((_, v)) = &shape.index {
                        return v.clone();
                    }
                    if shape.array.is_none() {
                        self.type_error(
                            format!(
                                "Key '{}' not found in table '{}'",
                                name,
                                Type::Table(Some(shape.clone()))
                            ),
                            loc,
                        );
                    }
                    return Type::Any;
                }
                if matches!(self.resolve_ty(key), Type::Number) {
                    if let Some(a) = &shape.array {
                        return Type::Optional(Box::new(a.clone()));
                    }
                }
                if let Some((_, v)) = &shape.index {
                    return v.clone();
                }
                Type::Any
            }
            _ => Type::Any,
        }
    }

    // --------------------------------------------------------- declarations

    /// `local a: T, b = e1, e2` just compiled its right-hand side: the top `n`
    /// types belong to the `n` newest locals. Annotated locals are checked;
    /// in strict mode unannotated ones take the initializer's type.
    pub(crate) fn tc_declare_locals(&mut self, n: usize, loc: TokenCell) {
        if n == 0 || self.locals.len() < n {
            return;
        }
        let first_local = self.locals.len() - n;
        let types: Vec<Type> = (0..n)
            .map(|i| {
                let len = self.tc.stack.len();
                if len >= n {
                    self.tc.stack[len - n + i].clone()
                } else {
                    Type::Any
                }
            })
            .collect();
        for (i, vty) in types.into_iter().enumerate() {
            // resolve first: an alias name is only "unknown" if it never resolves
            let declared = self.resolve_ty(&self.locals[first_local + i].ty);
            if declared.is_unknown() {
                if self.tc.strict && !vty.is_unknown() && !vty.is_nil() {
                    self.locals[first_local + i].ty = vty.widen();
                }
            } else {
                let len = self.tc.starts.len();
                self.tc.error_start = (len >= n).then(|| self.tc.starts[len - n + i]);
                self.check_assign(&declared, &vty, loc);
                self.tc.error_start = None;
            }
        }
    }

    // ------------------------------------------------------------ narrowing

    /// Inspect the condition just compiled (the tail of the chunk) for a shape
    /// that refines a local inside the branches.
    pub(crate) fn tc_narrow_from_condition(&mut self, f: &FunctionObject) -> Option<Narrow> {
        use OpCode::*;
        let code = &f.chunk.code;
        let n = code.len();
        let at = |i: usize| -> Option<&OpCode> {
            if i < n {
                Some(&code[n - 1 - i])
            } else {
                None
            }
        };
        let local_at = |i: usize| -> Option<u8> {
            match at(i) {
                Some(GET_LOCAL { index }) => Some(*index),
                _ => None,
            }
        };
        // type(x) == "name"
        if let (
            Some(EQUAL),
            Some(CONSTANT { constant }),
            Some(CALL(1, _, false)),
            Some(slot),
            Some(GET_GLOBAL { constant: g }),
        ) = (at(0), at(1), at(2), local_at(3), at(4))
        {
            if Self::const_name(f, *g).as_deref() == Some("type") {
                if let Some(name) = Self::const_name(f, *constant) {
                    if let Some(then_ty) = Type::from_type_name(&name) {
                        // on a union, each branch keeps the members that can reach it
                        let cur = self.resolve_ty(&self.local_ty(slot));
                        if let Some((yes, no)) = cur.split_type_name(&name) {
                            return self.make_narrow(slot, Some(yes), Some(no));
                        }
                        return self.make_narrow(slot, Some(then_ty), None);
                    }
                }
            }
        }
        // x ~= nil  /  x == nil
        if let (Some(cmp), Some(NIL), Some(slot)) = (at(0), at(1), local_at(2)) {
            let cur = self.local_ty(slot);
            let non_nil = cur.truthy();
            match cmp {
                NOT_EQUAL => return self.make_narrow(slot, Some(non_nil), Some(Type::Nil)),
                EQUAL => return self.make_narrow(slot, Some(Type::Nil), Some(non_nil)),
                _ => {}
            }
        }
        // if x then
        if let Some(slot) = local_at(0) {
            let cur = self.local_ty(slot);
            return self.make_narrow(slot, Some(cur.truthy()), None);
        }
        None
    }

    fn make_narrow(
        &mut self,
        slot: u8,
        then_ty: Option<Type>,
        else_ty: Option<Type>,
    ) -> Option<Narrow> {
        let idx = self.local_index(slot);
        let saved = self.locals.get(idx)?.ty.clone();
        // only refine something we actually know less about
        if saved.is_unknown() && then_ty.as_ref().map_or(true, |t| t.is_unknown()) {
            return None;
        }
        Some(Narrow {
            local: idx,
            saved,
            then_ty,
            else_ty,
        })
    }

    pub(crate) fn tc_narrow_then(&mut self, n: &Narrow) {
        self.tc.narrowed.push((n.local, n.saved.clone()));
        if let (Some(t), Some(l)) = (&n.then_ty, self.locals.get_mut(n.local)) {
            l.ty = t.clone();
        }
    }

    pub(crate) fn tc_narrow_else(&mut self, n: &Narrow) {
        if let Some(l) = self.locals.get_mut(n.local) {
            l.ty = n.else_ty.clone().unwrap_or_else(|| n.saved.clone());
        }
    }

    pub(crate) fn tc_narrow_end(&mut self, n: &Narrow) {
        if let Some(i) = self.tc.narrowed.iter().rposition(|(i, _)| *i == n.local) {
            self.tc.narrowed.remove(i);
        }
        if let Some(l) = self.locals.get_mut(n.local) {
            l.ty = n.saved.clone();
        }
    }

    // ----------------------------------------------------------- pre-scan

    /// Collect `type` aliases and named function signatures from the whole
    /// source before the main pass. Reads only; never emits or records errors.
    pub(crate) fn tc_prescan(&mut self, source: &str) {
        let lexer = Lexer::new(source);
        let mut it = lexer.peekable();
        let valid = self.valid;
        loop {
            if it.peek().is_none() {
                break;
            }
            let tok = match self.peek(&mut it) {
                Ok(t) => t.clone(),
                Err(_) => {
                    it.next();
                    continue;
                }
            };
            match tok {
                Token::EOF => break,
                Token::Identifier(s) if s == "type" => {
                    self.eat(&mut it);
                    if let Ok(Token::Identifier(name)) = self.peek(&mut it).cloned() {
                        self.eat(&mut it);
                        if matches!(self.peek(&mut it), Ok(Token::Assign)) {
                            self.eat(&mut it);
                            if let Ok(ty) = self.parse_type(&mut it) {
                                self.tc.aliases.insert(name, ty);
                            }
                        }
                    }
                }
                Token::Function => {
                    self.eat(&mut it);
                    if let Ok(Token::Identifier(name)) = self.peek(&mut it).cloned() {
                        self.eat(&mut it);
                        if matches!(self.peek(&mut it), Ok(Token::OpenParen)) {
                            self.eat(&mut it);
                            if let Ok(sig) = self.parse_sig_tail(&mut it) {
                                self.tc.fn_sigs.insert(name, sig);
                            }
                        }
                    }
                }
                _ => self.eat(&mut it),
            }
        }
        // `error_at` inside a failed type parse flips `valid`; the pre-scan
        // must not decide the compile's fate — and its token walk must not leave
        // position state behind for the main pass's spans.
        self.reset_scan_state(valid);
    }

    /// Undo what a side token walk (pre-scan, `declare`) did to the compiler's
    /// error and position state.
    fn reset_scan_state(&mut self, valid: bool) {
        self.valid = valid;
        self.errors.clear();
        self.current = Ok(Token::Nil);
        self.current_location = (0, 0);
        self.current_end = (0, 0);
        self.last_end = (0, 0);
        self.current_index = 0;
        self.span_starts.clear();
    }

    // -------------------------------------------------------------- snippets

    /// Decide whether the function whose scope just ended could be extracted
    /// as a snippet: every captured local must be a value, and every global it
    /// (or a nested function) touches must exist in a snippet VM — the standard
    /// library, a host declaration marked `declare_in_snippet`, or something we
    /// know nothing about (gradual). A user function or an undeclared host
    /// native would be `nil` there, so it is a provable `No`.
    pub(crate) fn tc_snippet_verdict(&self, state: &FunctionalState) -> Snippetness {
        for up in &state.up_values {
            let ty = self.resolve_ty(&up.ty);
            if is_reference(&ty) {
                return Snippetness::No(format!(
                    "it captures '{}' ({}); a snippet can only capture values",
                    up.name, ty
                ));
            }
        }
        for name in &state.referenced_globals {
            if self.tc.declared.in_snippet.contains(name) || is_standard_global(name) {
                continue;
            }
            let ty = self.resolve_ty(&self.global_ty(name));
            if is_reference(&ty) {
                let what = if self.tc.declared.globals.contains_key(name) {
                    "a host function not provided to snippets"
                } else {
                    "a function of this script"
                };
                return Snippetness::No(format!(
                    "it references global '{}', {}; it would be nil in the snippet VM",
                    name, what
                ));
            }
        }
        Snippetness::Yes
    }

    // ---------------------------------------------------------- declarations

    /// Declare the static type of a host-provided global, in annotation syntax:
    ///
    /// ```ignore
    /// compiler.declare("instr", "(number, snippet(number, number) -> number) -> nil")?;
    /// ```
    ///
    /// Declarations persist across compiles with this `Compiler`. A function
    /// literal passed where a `snippet` is expected is then checked against the
    /// snippet rules at compile time.
    pub fn declare(&mut self, name: &str, ty: &str) -> Result<(), String> {
        let ty = self.parse_type_str(ty)?;
        self.tc.declared.globals.insert(name.to_string(), ty);
        Ok(())
    }

    /// Like [`declare`](Self::declare), and additionally promise that the host
    /// registers this global inside every snippet VM too, so a snippet may call
    /// it (`emit`, `out`, …).
    pub fn declare_in_snippet(&mut self, name: &str, ty: &str) -> Result<(), String> {
        self.declare(name, ty)?;
        self.tc.declared.in_snippet.insert(name.to_string());
        Ok(())
    }

    /// Parse one type in annotation syntax.
    fn parse_type_str(&mut self, src: &str) -> Result<Type, String> {
        let valid = self.valid;
        let errors = std::mem::take(&mut self.errors);
        let lexer = Lexer::new(src);
        let mut it = lexer.peekable();
        let parsed = self.parse_type(&mut it);
        let trailing = !matches!(self.peek(&mut it), Ok(Token::EOF)) && it.peek().is_some();
        self.reset_scan_state(valid);
        self.errors = errors;
        match parsed {
            Ok(_) if trailing => Err(format!("unexpected trailing tokens in type '{}'", src)),
            Ok(t) => Ok(t),
            Err(e) => Err(format!("invalid type '{}': {}", src, e.code)),
        }
    }

    /// After `function name(`: the parameter list (names with optional
    /// annotations, `...`) up to and including `)`, then an optional `: R`.
    fn parse_sig_tail(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<FnSig, crate::error::ErrorTuple> {
        let mut sig = FnSig::default();
        loop {
            match self.peek(it)? {
                Token::CloseParen => {
                    self.eat(it);
                    break;
                }
                Token::Identifier(_) => {
                    self.eat(it);
                    let ty = if matches!(self.peek(it)?, Token::Colon) {
                        self.eat(it);
                        self.parse_type(it)?
                    } else {
                        Type::Any
                    };
                    sig.params.push(ty);
                }
                Token::VarArg => {
                    self.eat(it);
                    sig.variadic = true;
                    if matches!(self.peek(it)?, Token::Colon) {
                        self.eat(it);
                        self.parse_type(it)?;
                    }
                }
                Token::Comma => self.eat(it),
                other => {
                    let other = other.clone();
                    return Err(self.error_at(SiltError::InvalidTokenPlacement(other)));
                }
            }
        }
        if matches!(self.peek(it)?, Token::Colon) {
            self.eat(it);
            sig.returns = Some(self.parse_return_types(it)?);
        }
        Ok(sig)
    }

    // -------------------------------------------------------- type parser

    /// `type := '|'? postfix ('|' postfix)*`
    pub(crate) fn parse_type(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Type, crate::error::ErrorTuple> {
        // Luau allows a leading `|` (`type T = | A | B`, handy across lines)
        if matches!(self.peek(it)?, Token::Op(Operator::BitOr)) {
            self.eat(it);
        }
        let first = self.parse_type_postfix(it)?;
        if !matches!(self.peek(it)?, Token::Op(Operator::BitOr)) {
            return Ok(first);
        }
        let mut parts = vec![first];
        while matches!(self.peek(it)?, Token::Op(Operator::BitOr)) {
            self.eat(it);
            parts.push(self.parse_type_postfix(it)?);
        }
        Ok(Type::union(parts))
    }

    /// `postfix := atom '?'*`
    fn parse_type_postfix(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Type, crate::error::ErrorTuple> {
        let mut t = self.parse_type_atom(it)?;
        while matches!(self.peek(it)?, Token::Question) {
            self.eat(it);
            if !matches!(t, Type::Optional(_)) {
                t = Type::Optional(Box::new(t));
            }
        }
        Ok(t)
    }

    fn parse_type_atom(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Type, crate::error::ErrorTuple> {
        let (res, _) = self.pop(it);
        Ok(match res? {
            // `snippet(T, U) -> R`: a function type with the snippet rules attached
            Token::Identifier(name)
                if name == "snippet" && matches!(self.peek(it)?, Token::OpenParen) =>
            {
                self.eat(it);
                match self.parse_paren_type(it)? {
                    Type::Function(Some(mut sig)) => {
                        sig.snippet = Snippetness::Yes;
                        Type::Snippet(Some(sig))
                    }
                    _ => return Err(self.error_at(SiltError::ExpectedToken(Token::ArrowFunction))),
                }
            }
            Token::Identifier(name) => Type::from_name(&name),
            Token::Nil => Type::Nil,
            Token::StringLiteral(s) => Type::StringLit(s.to_string()),
            Token::True => Type::BoolLit(true),
            Token::False => Type::BoolLit(false),
            Token::Function => Type::Function(None),
            Token::OpenBrace => self.parse_table_type(it)?,
            Token::OpenParen => self.parse_paren_type(it)?,
            other => return Err(self.error_at(SiltError::InvalidTokenPlacement(other))),
        })
    }

    /// After `{`: `{}` | `{T}` | `{[K]: V}` | `{name: T, ...}`
    fn parse_table_type(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Type, crate::error::ErrorTuple> {
        let mut shape = TableShape::default();
        if matches!(self.peek(it)?, Token::CloseBrace) {
            self.eat(it);
            return Ok(Type::Table(Some(Box::new(shape))));
        }
        if matches!(self.peek(it)?, Token::OpenBracket) {
            self.eat(it);
            let k = self.parse_type(it)?;
            self.expect(it, Token::CloseBracket)?;
            self.expect(it, Token::Colon)?;
            let v = self.parse_type(it)?;
            shape.index = Some((k, v));
            self.eat_separator(it)?;
        }
        loop {
            match self.peek(it)? {
                Token::CloseBrace => {
                    self.eat(it);
                    break;
                }
                Token::Identifier(_) => {
                    let (res, _) = self.pop(it);
                    let name = match res? {
                        Token::Identifier(n) => n,
                        _ => unreachable!(),
                    };
                    if matches!(self.peek(it)?, Token::Colon) {
                        self.eat(it);
                        let t = self.parse_type(it)?;
                        shape.fields.push((name, t));
                    } else {
                        // `{T}` — an array of the named type (with any suffixes)
                        let mut t = Type::from_name(&name);
                        while matches!(self.peek(it)?, Token::Question) {
                            self.eat(it);
                            t = Type::Optional(Box::new(t));
                        }
                        // `{A | B}`
                        if matches!(self.peek(it)?, Token::Op(Operator::BitOr)) {
                            let mut parts = vec![t];
                            while matches!(self.peek(it)?, Token::Op(Operator::BitOr)) {
                                self.eat(it);
                                parts.push(self.parse_type_postfix(it)?);
                            }
                            t = Type::union(parts);
                        }
                        shape.array = Some(t);
                    }
                    self.eat_separator(it)?;
                }
                _ => {
                    // `{ {x: number} }`, `{ (number) -> string }`, …: an array of a complex type
                    let t = self.parse_type(it)?;
                    shape.array = Some(t);
                    self.eat_separator(it)?;
                }
            }
        }
        Ok(Type::Table(Some(Box::new(shape))))
    }

    fn eat_separator(&mut self, it: &mut Peekable<Lexer>) -> Result<(), crate::error::ErrorTuple> {
        match self.peek(it)? {
            Token::Comma | Token::SemiColon => {
                self.eat(it);
                Ok(())
            }
            Token::CloseBrace => Ok(()),
            other => {
                let other = other.clone();
                Err(self.error_at(SiltError::InvalidTokenPlacement(other)))
            }
        }
    }

    /// After `(`: a function type `(T, U) -> R` or a parenthesized type `(T)`.
    fn parse_paren_type(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Type, crate::error::ErrorTuple> {
        let mut params = vec![];
        let mut variadic = false;
        while !matches!(self.peek(it)?, Token::CloseParen) {
            if matches!(self.peek(it)?, Token::VarArg) {
                self.eat(it);
                variadic = true;
                if matches!(self.peek(it)?, Token::Colon) {
                    self.eat(it);
                    self.parse_type(it)?;
                }
            } else {
                params.push(self.parse_type(it)?);
            }
            if matches!(self.peek(it)?, Token::Comma) {
                self.eat(it);
            }
        }
        self.eat(it); // ')'
        if matches!(self.peek(it)?, Token::ArrowFunction) {
            self.eat(it);
            let returns = self.parse_return_types(it)?;
            return Ok(Type::Function(Some(Box::new(FnSig {
                params,
                variadic,
                returns: Some(returns),
                ..Default::default()
            }))));
        }
        if params.len() == 1 && !variadic {
            return Ok(params.pop().unwrap());
        }
        Err(self.error_at(SiltError::ExpectedToken(Token::ArrowFunction)))
    }

    /// `R` or `(R1, R2, ...)`
    pub(crate) fn parse_return_types(
        &mut self,
        it: &mut Peekable<Lexer>,
    ) -> Result<Vec<Type>, crate::error::ErrorTuple> {
        if matches!(self.peek(it)?, Token::OpenParen) {
            self.eat(it);
            let mut out = vec![];
            while !matches!(self.peek(it)?, Token::CloseParen) {
                out.push(self.parse_type(it)?);
                if matches!(self.peek(it)?, Token::Comma) {
                    self.eat(it);
                }
            }
            self.eat(it);
            // `(T) -> U` as a single return type, or a bare parenthesised type
            if matches!(self.peek(it)?, Token::ArrowFunction) {
                self.eat(it);
                let returns = self.parse_return_types(it)?;
                return Ok(vec![Type::Function(Some(Box::new(FnSig {
                    params: out,
                    variadic: false,
                    returns: Some(returns),
                    ..Default::default()
                })))]);
            }
            return Ok(out);
        }
        Ok(vec![self.parse_type(it)?])
    }

    fn expect(
        &mut self,
        it: &mut Peekable<Lexer>,
        tok: Token,
    ) -> Result<(), crate::error::ErrorTuple> {
        if self.peek(it)? == &tok {
            self.eat(it);
            Ok(())
        } else {
            Err(self.error_at(SiltError::ExpectedToken(tok)))
        }
    }
}

/// Is `name` a global the standard library installs (so a snippet VM has it)?
pub(crate) fn is_standard_global(name: &str) -> bool {
    crate::lua::STANDARD_GLOBALS.contains(&name)
}

/// Signatures of the standard-library globals worth knowing about.
fn builtin_global(name: &str) -> Option<Type> {
    let sig = |params: Vec<Type>, variadic: bool, returns: Vec<Type>| {
        Type::Function(Some(Box::new(FnSig {
            params,
            variadic,
            returns: Some(returns),
            ..Default::default()
        })))
    };
    Some(match name {
        "type" => sig(vec![Type::Any], false, vec![Type::String]),
        "tostring" => sig(vec![Type::Any], false, vec![Type::String]),
        "tonumber" => sig(
            vec![Type::Any, Type::Optional(Box::new(Type::Number))],
            false,
            vec![Type::Optional(Box::new(Type::Number))],
        ),
        "print" => sig(vec![], true, vec![]),
        _ => return None,
    })
}
