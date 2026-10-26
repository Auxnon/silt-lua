//! Snippets — see `SPEC-snippets.md`.
//!
//! A [`Snippet`] is a portable, arena-independent capture of a script-defined function: its
//! bytecode subtree plus `ExVal` snapshots of the value-typed state it needs (captured locals
//! and value globals). It holds no `Gc` from the source arena, so it is `Send + Sync` and can be
//! handed to another thread, which builds it into its own isolated VM ([`SnippetVm`]) and calls
//! it as often as it likes — a Lua function that has *escaped* the VM that defined it.
//!
//! The intended shape (a synth voice):
//!
//! ```lua
//! local gain = 0.3
//! instr(1, function(phase) return math.sin(phase) * gain end)   -- host native extracts a Snippet
//! ```
//!
//! The host native receives the closure `Value`, calls [`Snippet::extract`], and ships the
//! `Snippet` over a channel to the audio thread, which does `Lua::build_snippet(&s)` once and
//! then `vm.call_f64(&[phase])` per sample.

use crate::{
    chunk::Chunk,
    code::{ConstIndex, OpCode},
    error::SiltError,
    function::{Closure, FunctionObject, UpValue},
    lua::Lua,
    prelude::VM,
    value::{ExVal, Value},
};
use gc_arena::{lock::RefLock, Gc, Mutation};

/// Why a function could not be turned into a snippet. All are creation-time and explicit.
#[derive(Debug, Clone, PartialEq)]
pub enum SnippetError {
    /// The value handed in was not a function/closure.
    NotAFunction,
    /// A captured upvalue that escapes the snippet is a reference type (table with functions,
    /// closure, userdata, …) and cannot be cloned by value. Carries the upvalue slot index.
    EscapingReference(usize),
    /// A constant baked into the bytecode is a non-function reference that cannot be
    /// serialized.
    NonConstantConstant,
    /// A bare function *prototype* that captures upvalues was passed (not a live closure), so
    /// the captured values do not exist yet. Extract from the closure value instead.
    PrototypeOnly,
}

impl std::fmt::Display for SnippetError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SnippetError::NotAFunction => write!(f, "value is not a function"),
            SnippetError::EscapingReference(i) => write!(
                f,
                "captured upvalue #{} is a reference (function, userdata, or a table holding one) and cannot be cloned into a snippet",
                i
            ),
            SnippetError::NonConstantConstant => {
                write!(f, "a bytecode constant is a non-serializable reference")
            }
            SnippetError::PrototypeOnly => write!(
                f,
                "function prototype captures upvalues but is not a live closure; extract from the closure value"
            ),
        }
    }
}

impl std::error::Error for SnippetError {}

impl From<SnippetError> for SiltError {
    fn from(e: SnippetError) -> Self {
        SiltError::Custom(format!("snippet: {}", e))
    }
}

/// Arena-independent function prototype (bytecode + constants, no `Gc`).
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub enum SnippetConst {
    /// A primitive/value constant (number, string, …).
    Value(ExVal),
    /// A nested function constant — self-contained, recursively captured.
    Proto(SnippetProto),
}

/// A portable, arena-independent capture of a closure. Plain data: `Clone + Send + Sync`, no
/// lifetimes. Build it into a VM with [`Lua::build_snippet`] / [`SnippetVm::build`].
#[derive(Clone, Debug)]
pub struct Snippet {
    pub proto: SnippetProto,
    /// Escaping upvalues (locals of the defining scope the function closed over), value-copied
    /// in slot order.
    pub upvalues: Vec<ExVal>,
    /// Referenced globals that were value-typed → snapshotted.
    pub value_globals: Vec<(String, ExVal)>,
    /// Referenced globals that were reference-typed (stdlib or user) → resolve from the target VM
    /// at run time, `nil` if absent. Drives the dropped-reference error hints.
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
        let c = func.chunk.get_constant(i as ConstIndex);
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
    /// Extract a snippet from a live function value in VM `vm`. Upvalues the closure captured
    /// are read **now** (open or closed) and copied by value; a reference-typed capture is an
    /// [`SnippetError::EscapingReference`]. Globals are classified against `vm`'s global table.
    ///
    /// This is the call a host native makes on a function argument it was handed.
    pub fn extract<'gc>(vm: &VM<'gc>, value: &Value<'gc>) -> Result<Snippet, SnippetError> {
        let mut upvalues = Vec::new();
        let func: &FunctionObject = match value {
            Value::Closure(c) => {
                let stack = vm.stack_slice();
                for (i, up) in c.upvalues.iter().enumerate() {
                    let v = up.borrow().copy_value(stack);
                    match value_snapshot(&v) {
                        Some(ex) => upvalues.push(ex),
                        None => return Err(SnippetError::EscapingReference(i)),
                    }
                }
                &c.function
            }
            Value::Function(f) => {
                if f.upvalue_count > 0 {
                    return Err(SnippetError::PrototypeOnly);
                }
                f
            }
            _ => return Err(SnippetError::NotAFunction),
        };
        let proto = extract_proto(func)?;

        // Classify referenced globals.
        let mut names = Vec::new();
        collect_global_names(&proto, &mut names);
        let mut value_globals = Vec::new();
        let mut ref_globals = Vec::new();
        let globals = vm.globals.borrow();
        for name in names {
            match globals.get_str(name.as_str()) {
                Some(v) => match value_snapshot(v) {
                    Some(ex) => value_globals.push((name, ex)),
                    None => ref_globals.push(name),
                },
                None => ref_globals.push(name), // undefined in A → resolve from target (nil)
            }
        }

        Ok(Snippet {
            proto,
            upvalues,
            value_globals,
            ref_globals,
        })
    }

    /// Declared parameter count of the captured function.
    pub fn arity(&self) -> u8 {
        self.proto.arity
    }

    /// Whether the captured function takes `...`.
    pub fn is_variadic(&self) -> bool {
        self.proto.is_variadic
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
/// snippet's `value_globals` into `vm`'s global table (overlaying its stdlib) and rebuilds each
/// captured upvalue as a *closed* cell holding the snapshot.
pub fn instantiate<'g>(
    snippet: &Snippet,
    vm: &mut VM<'g>,
    mc: &Mutation<'g>,
) -> Result<Value<'g>, SiltError> {
    // Inject value-typed globals the snippet snapshotted.
    for (name, ex) in &snippet.value_globals {
        let v = ex.into_value(vm, mc)?;
        vm.globals.borrow_mut(mc).set(Value::string(mc, name.as_str()), v);
    }
    let func = materialize_proto(&snippet.proto, vm, mc)?;
    let mut upvalues = Vec::with_capacity(snippet.upvalues.len());
    for (i, ex) in snippet.upvalues.iter().enumerate() {
        let v = ex.into_value(vm, mc)?;
        // Never an open pointer into a stack: a closed cell owning the copied value.
        let mut up = UpValue::new(i as u8, 0);
        up.close_around(v);
        upvalues.push(Gc::new(mc, RefLock::new(up)));
    }
    let closure = Closure::new(func, upvalues);
    Ok(Value::Closure(Gc::new(mc, closure)))
}

/// A snippet built into its own isolated VM. Owns the arena; call it as often as you like.
/// Not `Send` (the arena is single-threaded) — build it **on the thread that will run it**
/// from the `Send` [`Snippet`].
pub struct SnippetVm {
    lua: Lua,
    arity: u8,
    ref_globals: Vec<String>,
    /// Calls since the last GC debt collection on the `f64` hot path.
    since_collect: u32,
}

/// How many `call_f64` calls go between GC debt collections.
const COLLECT_EVERY: u32 = 256;

impl SnippetVm {
    /// Build `snippet` into a fresh VM with the standard library (a few microseconds).
    pub fn build(snippet: &Snippet) -> Result<SnippetVm, String> {
        let mut lua = Lua::new_with_standard();
        lua.snippet_install(snippet)
            .map_err(|e| format!("[snippet] {}", e))?;
        Ok(SnippetVm {
            lua,
            arity: snippet.proto.arity,
            ref_globals: snippet.ref_globals.clone(),
            since_collect: 0,
        })
    }

    /// Call with `ExVal` arguments; the single result comes back as an `ExVal`.
    pub fn call(&mut self, args: &[ExVal]) -> Result<ExVal, String> {
        let r = self.lua.snippet_call(args);
        r.map_err(|e| self.describe(e))
    }

    /// Hot path: call with `f64` arguments and read one number back. No marshalling, no heap
    /// allocation beyond the VM stack. Integers are widened; a non-numeric result is an error.
    #[inline]
    pub fn call_f64(&mut self, args: &[f64]) -> Result<f64, String> {
        self.since_collect += 1;
        if self.since_collect >= COLLECT_EVERY {
            self.since_collect = 0;
            self.lua.collect();
        }
        match self.lua.snippet_call_f64(args) {
            Ok(v) => Ok(v),
            Err(e) => Err(self.describe(e)),
        }
    }

    /// Fill `out` by calling the snippet once per sample with `(index, sample_rate)` mapped
    /// through `args_for` — the audio-buffer shape: `vm.render(&mut buf, |i| [phase(i)])`.
    pub fn render<const N: usize>(
        &mut self,
        out: &mut [f32],
        mut args_for: impl FnMut(usize) -> [f64; N],
    ) -> Result<(), String> {
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.call_f64(&args_for(i))? as f32;
        }
        Ok(())
    }

    /// Declared parameter count of the snippet function.
    pub fn arity(&self) -> u8 {
        self.arity
    }

    /// The underlying VM, e.g. to register host natives the snippet may call
    /// (`vm.lua().enter(|vm, mc| vm.register_native_function(mc, "out", …))`).
    pub fn lua(&mut self) -> &mut Lua {
        &mut self.lua
    }

    /// Every error out of a snippet is tagged `[snippet]` so it is never mistaken for an error
    /// in the calling script. If the failure smells like a dropped reference (a nil where the
    /// defining script had a live function/table), name the globals that could be responsible.
    fn describe(&self, e: SiltError) -> String {
        let msg = format!("{}", e);
        if !self.ref_globals.is_empty() && msg.contains("nil") {
            format!(
                "[snippet] {} — note: {} {} a reference in the defining script and not copied into the snippet; it resolves from the snippet VM (nil if absent). Provide it there or pass it by value.",
                msg,
                self.ref_globals
                    .iter()
                    .map(|g| format!("'{}'", g))
                    .collect::<Vec<_>>()
                    .join(", "),
                if self.ref_globals.len() == 1 { "was" } else { "were each" }
            )
        } else {
            format!("[snippet] {}", msg)
        }
    }
}
