# Spec: snippets — a Lua function that escapes the VM

Status: **implemented** (stages 1–4; Rust embedding API) · Feature gate: `snippets`
Code: `src/snippet.rs`, `Lua::{extract_snippet, build_snippet, run_snippet}` in `src/lua.rs`,
tests in `tests/snippets.rs`, bench `snippet_call_f64_2e5` in `benches/interpreter.rs`.

## Concept

A **snippet** is a portable, self-contained capture of a script-defined function: its bytecode
subtree plus a snapshot of the *value-typed* state it needs, with every reference to the
originating VM's live objects severed. It is plain data (`Clone + Send + Sync`, no lifetimes), so
it can be queued, stored, or handed to **another thread**, which builds it into an independent VM
(its own gc-arena) and calls it as often as it likes — a lightweight, cleanly-separated
alternative to coroutines for "define here, run elsewhere". No yield/resume, no shared stack: a
snippet runs to completion in its own VM. That separation is the whole point.

The motivating use is a synth: a script defines a voice as a function, the engine ships it to the
audio thread, and the audio thread calls it once per sample in a VM it owns.

```lua
-- in the game script (VM A, main/worker thread)
local gain = 0.3
instr(1, function(phase, freq)            -- host native: Snippet::extract(vm, &f)
    return math.sin(phase) * gain         -- `gain` is captured BY VALUE; `math` resolves in B
end)
```

```rust
// in the host, audio thread (VM B)
let mut voice = Lua::build_snippet(&snippet)?;   // ~5 µs, once
let s = voice.call_f64(&[phase, freq])?;         // ~0.2 µs, per sample
```

## Surface

**Rust API (the real one):**

| | |
|---|---|
| `Snippet::extract(&VM, &Value) -> Result<Snippet, SnippetError>` | from a function `Value` — what a host native does with a function argument it was handed. Reads captured upvalues *now* (open or closed). |
| `Lua::extract_snippet(name, src, &mut Compiler) -> Result<Snippet, String>` | convenience: **runs** `src` in this VM and extracts the function it `return`s (`local k = 2  return function(x) return x * k end`). |
| `Lua::build_snippet(&Snippet) -> Result<SnippetVm, String>` | fresh isolated VM + stdlib, snippet materialized. Build it on the thread that will run it. |
| `SnippetVm::call(&[ExVal]) -> Result<ExVal, String>` | general call, one result. |
| `SnippetVm::call_f64(&[f64]) -> Result<f64, String>` | hot path: numbers in, one number out, no marshalling or heap allocation beyond the VM stack. |
| `SnippetVm::render(&mut [f32], \|i\| [args…])` | fill an audio buffer, one call per sample. |
| `SnippetVm::lua() -> &mut Lua` | the VM, to register host natives the snippet expects (`emit`, `out`, …). |
| `Lua::run_snippet(&Snippet, Vec<ExVal>)` | one-shot build + call. |

`SnippetError` converts into `SiltError`, so a native can `Snippet::extract(vm, &f)?`.

**No Lua-side `snippet()` builtin.** The host native that *consumes* the function is the natural
surface (`instr(id, fn)`); a generic handle type would just defer the same question to the host.
Can be added later as userdata wrapping a `Snippet` if a script-side use appears.

## Threading model (decided by the types)

- `Snippet` is `Send + Sync` (bytecode + `ExVal`s). Ship it anywhere.
- `Lua` / `SnippetVm` are **not** `Send`: gc-arena `Gc` pointers are thread-bound by design, and
  natives registered into a VM may capture non-`Send` state, so an `unsafe impl Send` would be
  unsound in general. Hence: *build on the consuming thread*. Building costs a few microseconds
  (`Lua::new_with_standard` ≈ 7 µs, `build_snippet` ≈ 5 µs on a desktop), so a stdlib "extent"
  trimming step — proposed in the first draft — was dropped as pointless.

## Data model

```rust
pub struct Snippet {
    pub proto: SnippetProto,                 // root prototype tree (frozen code)
    pub upvalues: Vec<ExVal>,                // escaping captures, value-copied, slot order
    pub value_globals: Vec<(String, ExVal)>, // referenced globals that were VALUES → snapshot
    pub ref_globals: Vec<String>,            // referenced globals that were REFERENCES (stdlib or
                                             // user) → resolve from the target VM at run time,
                                             // nil if absent; drives the error hints
}
pub struct SnippetProto { code: Vec<OpCode>, constants: Vec<SnippetConst>, arity, upvalue_count,
                          is_variadic, varidic_index, name }
pub enum SnippetConst { Value(ExVal), Proto(SnippetProto) }   // nested functions recurse
```

## Extraction rules

1. The value must be a `Closure` (or a bare `Function` prototype with no upvalues; one *with*
   upvalues is `PrototypeOnly` — the captured values don't exist yet).
2. **Prototype tree**: `code` copied verbatim; constants are primitives (`ExVal`) or nested
   function prototypes (recurse). Any other reference constant → `NonConstantConstant`.
3. **Escaping upvalues** (the closure's own `upvalues`, i.e. locals of the defining scope it
   closed over): read the current value — from the live stack slot if still open, else the closed
   cell — and snapshot it. Value types and **pure-data tables** (deep-copied) are fine; a
   function, userdata, or a table containing one → `EscapingReference(slot)`. Nested functions'
   *internal* upvalues (captured from the snippet's own locals) are not snapshotted: the normal
   `CLOSURE`/`REGISTER_UPVALUE` machinery recreates them at run time.
4. **Globals**: scan the tree for `GET/SET/DEFINE_GLOBAL` names. Value → `value_globals`
   (pure-data tables deep-copied: a config/lookup table travels); reference (`math`, `print`, a
   user function, userdata) or undefined → `ref_globals`.

A snapshot is a **copy**: mutating a captured upvalue or a snapshotted global inside the snippet
persists across calls *in that SnippetVm* but never reaches the defining VM, and a second build
starts from the snapshot again.

## Instantiation

Fresh `Lua::new_with_standard()`; `value_globals` injected over the stdlib; the prototype tree
rebuilt into the new arena; each captured upvalue rebuilt as a **closed** cell holding the copy
(never an open pointer into a stack); the resulting closure stored in the VM's `snippet_closure`
slot and invoked through `call_protected` (which also serves `pcall`).

## Error model

Because a dropped reference silently becomes `nil`, a snippet can fail in a way that would be
impossible in the defining script (`helper(x)` → "attempt to call a nil value"). Two layers:

1. **Provenance tag**: every error out of a snippet is prefixed `[snippet]` so the host/editor
   never attributes it to the calling script.
2. **Dropped-reference hint**: if the message involves `nil` and the snippet has `ref_globals`,
   the error names them: *"'helper' was a reference in the defining script and not copied into the
   snippet; it resolves from the snippet VM (nil if absent). Provide it there or pass it by
   value."* Best-effort naming on top of the always-correct tag. (A per-`GET_GLOBAL` stamp for
   exact attribution was considered and skipped — the name list is almost always one entry.)

Creation-time errors are explicit: `NotAFunction`, `EscapingReference(i)`, `NonConstantConstant`,
`PrototypeOnly`.

## Static checking (`typing` feature)

Nothing in the source marks a function as a snippet; the consumer decides. With `typing` the
host can say so at the type level and the rules above become compile errors — in the editor,
before the audio thread ever sees the function:

```rust
compiler.declare("instr", "(number, snippet(number, number) -> number) -> nil")?;
compiler.declare_in_snippet("emit", "(number) -> nil")?;   // registered in every SnippetVm
```

```lua
--!strict
local gain = 0.3
local shape = function(x) return x * x end
instr(1, function(p, f) return shape(p) * gain end)
--        ^ Type '(any, any) -> any' could not be converted into
--          'snippet(number, number) -> number': it captures 'shape' ((any) -> any);
--          a snippet can only capture values (argument 2)
```

(`--!strict` so the unannotated `shape` gets its inferred type; in nonstrict mode annotate it
or it stays `any` and passes gradually. A root-level `local function shape` is a *global* in
silt and gets the global message instead: *"it references global 'shape', a function of this
script; it would be nil in the snippet VM"*.)

`snippet` is a type (`Type::Snippet`), a subtype of `function`: a snippet goes anywhere a
function does; a function literal goes where a snippet is expected only if the checker could
not prove it is not one. The verdict is computed for every function literal from the compiler's
own upvalue list and the globals the body (and nested bodies) touch, mirroring extraction rules
3–4: a captured local whose static type is a function (or a table shape holding one) is an
`EscapingReference` in waiting; a referenced global that is a script function or an undeclared
host native would be `nil` in the snippet VM. The standard library, `declare_in_snippet`
natives and unknown names pass — gradual, like the rest of the checker. Signature and return
types are checked as for any function type. Value-dependent cases (an `any` capture, a table
that only sometimes holds a function) remain runtime `[snippet]` errors.

The language server reads the declarations from `initializationOptions` (`declare`,
`declareInSnippet`); `lsp::diagnostics_with` is the in-process form. Bytecode and `type(f)`
are unchanged — a snippet is a function in every sense.

## Performance (release, desktop)

| | |
|---|---|
| `Lua::new_with_standard()` | ≈ 6.8 µs |
| `build_snippet` | ≈ 5.4 µs |
| `SnippetVm::call` (`Vec<ExVal>` in/out) | ≈ 255 ns / call |
| `SnippetVm::call_f64` | see `cargo bench --features snippets snippet` |

At 48 kHz one voice is ~48k calls/s ≈ 1–1.5 % of a core via the fast path; polyphony scales
linearly. For heavier work, give the snippet a buffer-filling native via `SnippetVm::lua()`.

## What landing this required elsewhere

Testing snippets exposed three old VM/compiler bugs (fixed in a separate PR, stacked under this
one): functions nested ≥ 3 deep capturing a grandparent local addressed their own params one slot
low; `pcall`/`call_protected` panicked on a variadic callee and read stale slots for omitted
params; a multi-value `return` at the root yielded its last value instead of its first.

## Staging (all done)

1. Pure snippets — proto tree copied cross-arena, globals classified, materialized + run.
2. Escaping value upvalues — snapshot + closed-cell rebuild.
3. Global snapshot + target-resolved references, with the tagged/hinted error model.
4. Nested functions inside the snippet (recursive proto copy; internal upvalues at run time),
   variadic snippets, `SnippetVm` with the `f64` hot path and `render`, cross-thread tests.

## Open / future

- **Lua-side handle** (`snippet(fn)` → userdata) if a script ever needs to hold one.
- **Runtime provenance bit**: a flag on the prototype saying the literal was typed as a
  snippet, so `Snippet::extract` could refuse untyped functions for a host that wants
  strictness. Not needed while the type check runs before the code does.
- **A `snippet(...) ... end` literal** as syntax sugar for the annotation. Would reserve a word;
  the annotation (`local v: snippet = function ... end`) and the declared consumer cover the
  use so far.
- **Serde**: `Snippet` is all owned data (`OpCode`, `ExVal`); deriving `Serialize` would let a
  snippet cross a process/worker boundary (e.g. a browser AudioWorklet fed over MessagePack).
  That worklet would then need the silt VM compiled into it — a size trade-off for the host.
- **Mutable shared state** is deliberately out of scope; use the host's own channels.
- `Lua: Send` for the *snippet VM only* would need a VM variant that statically forbids
  non-`Send` natives — not worth it while "build on the consuming thread" costs 5 µs.
