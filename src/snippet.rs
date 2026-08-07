//! Snippets — see `SPEC-snippets.md`.
//!
//! A [`Snippet`] is a portable, arena-independent capture of a script-defined function: its
//! bytecode subtree plus `ExVal` snapshots of the value-typed state it needs. It holds no `Gc`
//! from the source arena, so it can be handed to and run in an independent VM.
//!
//! Stage 1 (this file): pure functions — no captured upvalues. The prototype tree is copied out
//! of the compiled function, globals are classified (value → snapshot, reference → resolve from
//! the target), and the result is re-materialized into a fresh arena and run. Upvalue capture
//! (which needs runtime values and careful closed-upvalue handling) is the next increment.

use crate::{
    chunk::Chunk,
    code::OpCode,
    error::SiltError,
    function::{Closure, FunctionObject},
    prelude::VM,
    value::{ExVal, Value},
};
use gc_arena::{Gc, Mutation};

/// Why a function could not be turned into a snippet. All are creation-time and explicit.
#[derive(Debug, Clone, PartialEq)]
pub enum SnippetError {
    /// The value handed in was not a function/closure.
    NotAFunction,
    /// A captured upvalue that escapes the snippet is a reference type (table, closure, …) and
    /// cannot be cloned by value. Carries the upvalue slot index.
    EscapingReference(usize),
    /// A constant baked into the bytecode is a non-function reference (e.g. an embedded native
    /// function) that cannot be serialized.
    NonConstantConstant,
    /// Stage-1 limitation: the function captures upvalues (not yet supported).
    UpvaluesNotYetSupported,
}

impl std::fmt::Display for SnippetError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SnippetError::NotAFunction => write!(f, "value is not a function"),
            SnippetError::EscapingReference(i) => {
                write!(f, "captured upvalue #{} is a reference and cannot be cloned into a snippet", i)
            }
            SnippetError::NonConstantConstant => {
                write!(f, "a bytecode constant is a non-serializable reference")
            }
            SnippetError::UpvaluesNotYetSupported => {
                write!(f, "snippet captures upvalues (not supported yet)")
            }
        }
    }
}

/// Arena-independent function prototype (bytecode + constants, no `Gc`).
#[derive(Clone)]
pub struct SnippetProto {
    pub code: Vec<OpCode>,
    pub constants: Vec<SnippetConst>,
    pub arity: u8,
    pub upvalue_count: u8,
    pub is_variadic: bool,
    pub varidic_index: u8,
    pub name: Option<String>,
}

/// A bytecode constant, in arena-independent form.
#[derive(Clone)]
pub enum SnippetConst {
    /// A primitive/value constant (number, string, …).
    Value(ExVal),
    /// A nested function constant — self-contained, recursively captured.
    Proto(SnippetProto),
}

/// A portable, arena-independent capture of a closure. Plain data: `Clone`, no lifetimes.
#[derive(Clone)]
pub struct Snippet {
    pub proto: SnippetProto,
    /// Escaping upvalues, value-copied (empty in stage 1).
    pub upvalues: Vec<ExVal>,
    /// Referenced globals that were value-typed → snapshotted.
    pub value_globals: Vec<(String, ExVal)>,
    /// Referenced globals that were reference-typed (stdlib or user) → resolve from the target VM
    /// at run time, `nil` if absent. Drives std-extent and the dropped-reference error hints.
    pub ref_globals: Vec<String>,
}

/// Strict value snapshot: value types (incl. pure-data tables) → `Some(ExVal)`; reference types
/// (function/closure/native/userdata, or a table containing any of those) → `None`.
fn value_snapshot(v: &Value) -> Option<ExVal> {
    match v {
        Value::Nil
        | Value::Integer(_)
        | Value::Number(_)
        | Value::Bool(_)
        | Value::Infinity(_)
        | Value::String(_) => Some(v.clone().into()),
        #[cfg(feature = "vector")]
        Value::Vec2(_) | Value::Vec3(_) | Value::Vec4(_) => Some(v.clone().into()),
        Value::Table(t) => {
            // pure-data deep copy: `to_exval` converts references to `Meta`/`UserData`
            // placeholders, so a table is only clonable if the result contains none.
            let ex = ExVal::Table(t.borrow().to_exval());
            if exval_is_pure(&ex) {
                Some(ex)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// True if an `ExVal` tree contains no reference placeholders (`Meta`/`UserData`).
fn exval_is_pure(ex: &ExVal) -> bool {
    match ex {
        ExVal::Meta(_) | ExVal::UserData(_) => false,
        ExVal::Table(t) => t.iter().all(|(k, v)| exval_is_pure(k) && exval_is_pure(v)),
        _ => true,
    }
}

/// Copy a compiled function's prototype tree into arena-independent form.
pub fn extract_proto(func: &FunctionObject) -> Result<SnippetProto, SnippetError> {
    let n = func.chunk.constants_len();
    let mut constants = Vec::with_capacity(n);
    for i in 0..n {
        let c = func.chunk.get_constant(i as u8);
        constants.push(match c {
            Value::Function(nested) => SnippetConst::Proto(extract_proto(nested)?),
            other => match value_snapshot(other) {
                Some(ex) => SnippetConst::Value(ex),
                None => return Err(SnippetError::NonConstantConstant),
            },
        });
    }
    Ok(SnippetProto {
        code: func.chunk.code.clone(),
        constants,
        arity: func.arity,
        upvalue_count: func.upvalue_count,
        is_variadic: func.is_variadic,
        varidic_index: func.varidic_index,
        name: func.name.clone(),
    })
}

/// Collect the distinct global names a prototype tree references (get/set/define).
fn collect_global_names(proto: &SnippetProto, out: &mut Vec<String>) {
    for op in &proto.code {
        let ci = match op {
            OpCode::GET_GLOBAL { constant }
            | OpCode::SET_GLOBAL { constant }
            | OpCode::DEFINE_GLOBAL { constant } => *constant as usize,
            _ => continue,
        };
        if let Some(SnippetConst::Value(ExVal::String(s))) = proto.constants.get(ci) {
            if !out.contains(s) {
                out.push(s.clone());
            }
        }
    }
    for c in &proto.constants {
        if let SnippetConst::Proto(p) = c {
            collect_global_names(p, out);
        }
    }
}

impl Snippet {
    /// Extract a snippet from a live function value in VM `vm`. Stage 1 requires no captured
    /// upvalues; globals are classified against `vm`'s global table.
    pub fn extract(vm: &VM, value: &Value) -> Result<Snippet, SnippetError> {
        let func: &FunctionObject = match value {
            Value::Closure(c) => {
                if !c.upvalues.is_empty() {
                    return Err(SnippetError::UpvaluesNotYetSupported);
                }
                &c.function
            }
            Value::Function(f) => f,
            _ => return Err(SnippetError::NotAFunction),
        };
        if func.upvalue_count > 0 {
            return Err(SnippetError::UpvaluesNotYetSupported);
        }
        let proto = extract_proto(func)?;

        // Classify referenced globals.
        let mut names = Vec::new();
        collect_global_names(&proto, &mut names);
        let mut value_globals = Vec::new();
        let mut ref_globals = Vec::new();
        let globals = vm.globals.borrow();
        for name in names {
            match globals.get(name.as_str()) {
                Some(v) => match value_snapshot(v) {
                    Some(ex) => value_globals.push((name, ex)),
                    None => ref_globals.push(name),
                },
                None => ref_globals.push(name), // undefined in A → resolve from target (nil)
            }
        }

        Ok(Snippet {
            proto,
            upvalues: Vec::new(),
            value_globals,
            ref_globals,
        })
    }
}

/// Re-materialize a prototype tree into `vm`'s arena as a `FunctionObject`.
fn materialize_proto<'g>(
    proto: &SnippetProto,
    vm: &mut VM<'g>,
    mc: &Mutation<'g>,
) -> Result<Gc<'g, FunctionObject<'g>>, SiltError> {
    let mut chunk = Chunk::new();
    // Push constants in order so their indices line up with the copied bytecode.
    for c in &proto.constants {
        let v = match c {
            SnippetConst::Value(ex) => ex.into_value(vm, mc)?,
            SnippetConst::Proto(p) => Value::Function(materialize_proto(p, vm, mc)?),
        };
        chunk.write_constant(v);
    }
    chunk.code = proto.code.clone();

    let mut func = FunctionObject::new(proto.name.clone(), false);
    func.arity = proto.arity;
    func.upvalue_count = proto.upvalue_count;
    func.is_variadic = proto.is_variadic;
    func.varidic_index = proto.varidic_index;
    func.set_chunk(chunk);
    Ok(Gc::new(mc, func))
}

/// Instantiate a snippet into `vm`'s arena, returning a callable `Value::Closure`. Injects the
/// snippet's `value_globals` into `vm`'s global table (overlaying its stdlib).
pub fn instantiate<'g>(
    snippet: &Snippet,
    vm: &mut VM<'g>,
    mc: &Mutation<'g>,
) -> Result<Value<'g>, SiltError> {
    if !snippet.upvalues.is_empty() {
        return Err(SiltError::Custom(
            "snippet upvalues not yet supported".to_string(),
        ));
    }
    // Inject value-typed globals the snippet snapshotted.
    for (name, ex) in &snippet.value_globals {
        let v = ex.into_value(vm, mc)?;
        vm.globals.borrow_mut(mc).set(name.clone(), v);
    }
    let func = materialize_proto(&snippet.proto, vm, mc)?;
    let closure = Closure::new(func, Vec::new());
    Ok(Value::Closure(Gc::new(mc, closure)))
}
