# Spec: snippets — extract a script function into an isolated VM

Status: **proposed** · Feature gate: `snippets` (reuses `hot-swap` subtree-walk internals)

**Surface (now): Rust embedding API only** — `Lua::extract_snippet` / `Lua::run_snippet`. No Lua
`snippet()` builtin and no userdata handle yet; a `Snippet` is a plain Rust value the host holds and
runs. (The Lua-facing surface is a later add once the Rust round-trip is proven.)

## Concept

A **snippet** is a portable, self-contained capture of a script-defined function: its bytecode
subtree plus a snapshot of the *value-typed* state it needs, with every reference to the
originating VM's live objects severed. It can be instantiated and run in an **independent VM (its
own gc-arena)**, decoupled from the script that defined it — a lightweight, cleanly-separated
alternative to coroutines for "define here, run elsewhere." No yield/resume, no shared stack: a
snippet runs to completion in its own VM. That separation is the whole point.

```lua
-- in script A
local factor = 3                        -- a value upvalue (escapes into the snippet)
local s = snippet(function(x)
    return x * factor                   -- captures `factor` by value; no ref to A's live state
end)
-- `s` can now be shipped to and run in a *different* VM, with no tie to A's compiled code
```

## Why build on the hot-swap system

The hot-swap engine already:
- walks a function's nested-prototype subtree by scanning `CLOSURE` opcodes (`nested_prototypes`,
  `plan_swaps` in `lua.rs`),
- maps compiled `FunctionObject`s to their source ranges,
- understands the shared-prototype + per-closure-upvalue model.

Snippet extraction is the **inverse of a swap**: instead of redirecting a live prototype to new
code, we deep-copy a prototype subtree *out* into arena-independent form. Same subtree walk, same
prototype model — so this extends existing machinery rather than inventing a parallel one.

## The core challenge: cross-arena copy

`Value<'gc>` and every `Gc<'gc, _>` are branded to a single arena (`Lua { arena: Arena<Rootable![VM]> }`).
A snippet **must hold no `Gc` from the source arena**. So a snippet is an arena-independent,
lifetime-free plain-data structure, deep-copied from the source closure and re-materialized into the
target arena on instantiation.

What crosses cleanly and what doesn't:
- **Bytecode** (`Chunk.code: Vec<OpCode>`) — already arena-independent plain data. Copied verbatim.
- **Constants** (`Chunk.constants: Vec<Value>`) — may hold nested `Function` prototypes (self-contained,
  recurse) or primitives (copy). Any *other* reference type is rejected.
- **Upvalues** and **globals** — the only arena-bound state the body actually depends on; these are
  exactly what we snapshot-or-reject (rules below).

`ExVal` (the existing arena-independent value type: primitives + `ExTable`) is the value-copy +
reference-detection boundary — reference types (`Closure`/`Function`/`NativeFunction`/`UserData`)
have no `ExVal` form, which is precisely the "fail if it's a reference" line.

## Data model (arena-independent)

```rust
pub struct Snippet {
    proto: SnippetProto,                    // root function prototype tree
    upvalues: Vec<ExVal>,                    // escaped upvalues, value-copied (ref => creation error)
    value_globals: Vec<(String, ExVal)>,     // referenced globals that were VALUES → snapshotted
    ref_globals: Vec<String>,                // referenced globals that were REFERENCES (stdlib or
                                             // user) → not copied; resolve from the target VM at run
                                             // time, nil if absent. Drives std-extent + error hints.
    std_extent: StdExtent,                   // which stdlib modules the target must load (minimize startup)
}
```

`ref_globals` unifies "stdlib the target already has" and "user references we dropped": at creation
we can't tell which the target will provide, so both are just *names to resolve later*. If the target
supplies it (its own `print`, or an injected value) the snippet uses it; if not, it's plain `nil`
(Lua semantics) — **no fail at creation, no fail at instantiation**, per your call.

pub struct SnippetProto {
    code: Vec<OpCode>,
    constants: Vec<SnippetConst>,
    arity: u8,
    upvalue_count: u8,
    is_variadic: bool,
    varidic_index: u8,
    // hot-swap redirect cell + source lines are dropped; a snippet is frozen code
}

pub enum SnippetConst {
    Value(ExVal),        // primitive constant
    Proto(SnippetProto), // nested function constant — self-contained
}
```

`Snippet` is `Clone + Send`-able plain data (no `'gc`), so it can be stored, queued, or handed to
another thread/VM. (Serde is a trivial follow-up since it's all owned data + `ExVal`, which already
has a serialize path on the dual-array branch.)

## Creation — in source VM A: `VM::extract_snippet(closure) -> Result<Snippet, SnippetError>`

1. Require a `Value::Closure` (or bare `Function` with no upvalues).
2. **Copy the prototype tree** (reuse the `nested_prototypes` walk):
   - `code` copied verbatim.
   - each constant: primitive → `SnippetConst::Value(value.to_exval())`; nested `Function` → recurse
     into `SnippetConst::Proto`; **any other reference constant → `NonConstantConstant` error**
     (a `NativeFunction` baked as a constant can't be serialized).
3. **Escape analysis for upvalues.** The root closure's `upvalues` are the values captured from A's
   enclosing scope — i.e. the ones that *escape* into the snippet. For each, read its current value
   (`UpValue::copy_value`) and convert to `ExVal`; a **reference type → `EscapingReference` error**.
   Nested functions' *own* upvalues (captured from the snippet's own locals — "upvalues inside the
   functional scope") are **not** snapshotted: they're re-created at run time by the normal
   `CLOSURE`/`REGISTER_UPVALUE` machinery. Only escaping upvalues are restricted to copyable values.
4. **Global snapshot.** Scan the proto tree for `GET_GLOBAL`/`SET_GLOBAL` constant names; for each
   distinct name look it up in A's globals:
   - value type → `value_globals.push((name, exval))` (pure copy — "functionally constant"),
   - **table** → attempt a strict **pure-data deep copy** (`to_exval`, but rejecting any reference
     leaf): if the whole table is data → `value_globals` (deep-copied `ExVal::Table`); if it holds
     references (e.g. a stdlib table like `math`, full of native fns) → `ref_globals` (resolve from
     the target). So a user's config/lookup **global table is deep-copied**; `math`/`string` resolve
     from the target's own stdlib.
   - native fn / closure / userdata → `ref_globals.push(name)` (resolved from the target, nil if absent).

   (Escaping **upvalues** stay strict: a reference upvalue — including a table — hard-fails at
   creation, because it isn't named and can't be resolved or meaningfully deep-copied as shared state.
   Only *global* tables, treated as functionally-constant data, are deep-copied.)
5. **Derive `std_extent`.** Intersect `ref_globals` with the known stdlib module names (`math`,
   `string`, `table`, `os`, base functions like `print`/`pairs`/…). The result is the *minimum*
   standard library the target VM must load — so a snippet that only does arithmetic spins up a bare
   VM, while one that calls `string.format` pulls in `string`. (Author can also widen it explicitly.)

## Instantiation + run — in target VM B: `VM::load_snippet(&Snippet) -> Value::Closure`

0. Build B with only `snippet.std_extent`'s modules loaded (not full `new_with_standard`) to minimize
   startup — a pure-arithmetic snippet gets a bare VM.
1. Materialize the proto tree into B's arena: rebuild each `FunctionObject` (code copied; constants
   rebuilt — `ExVal → Value` in B, nested `Proto → Function`), wrapped in fresh `Gc`s, each with
   `from_snippet = true`.
2. Rebuild the root closure's upvalues: for each snapshot `ExVal`, create a **closed** `UpValue`
   holding the copied value (never an open pointer into a stack).
3. Inject `value_globals` into B's global table (overlaying B's std_extent modules). `ref_globals` are
   left to resolve from B naturally — present if B has them, `nil` otherwise (with the error model above).
4. Return a `Value::Closure`, callable in B like any other function. Results marshal back out as
   `ExVal` (the natural cross-arena boundary).

Isolation: B is its own arena, provides its *own* (minimal) standard library — a snippet that calls
`print` uses B's `print`, not A's. Reuse a sandbox VM per snippet or pool them (Petrichor's call).

## Reference-typed globals (decided)

Snapshot value globals; reference globals (stdlib *or* user) are **not** copied — they resolve from
the target VM at run time and are plain `nil` if absent. No creation/instantiation failure for a
missing reference; it degrades to Lua's normal nil behavior. The target's std-extent is set from the
snippet's `std_extent` so only the needed modules load.

## Error model — the subtle part

Because a dropped reference silently becomes `nil`, a snippet can fail in a way that would be
*impossible in the defining script*: `foo()` where `foo` was a live global in A errors with "attempt
to call a nil value" in the snippet. Without help, that error is deeply misleading. Two mechanisms:

1. **Provenance tag — every error out of a snippet is marked.** Materialized snippet
   `FunctionObject`s carry a `from_snippet: true` flag (and the running VM knows it's executing a
   snippet). Any `SiltError` raised while executing snippet code is wrapped so the surfaced message
   is unmistakably "*error inside a snippet*", not attributed to the calling script's line numbers.
   Concretely: an `ErrorOut { snippet: true, .. }` marker + a `[snippet]` prefix, so the host/editor
   never confuses snippet failures with main-script failures.

2. **Dropped-reference amendment — name the nil.** The snippet carries `ref_globals` (names that were
   references in A). When a nil-operation error fires inside a snippet (call / index / arithmetic /
   concat on nil), and the offending nil traces to a `GET_GLOBAL(name)` with `name ∈ ref_globals`,
   amend the message:

   > attempt to call a nil value (global 'foo') — 'foo' was a **reference** in the defining script and
   > could not be cloned into the snippet; provide it in the target environment or pass it by value.

   Implementation: keep nil semantics (so `if foo then` still behaves like nil), but in snippet mode
   have `GET_GLOBAL` stamp the resolved name onto the frame when it yields nil for a `ref_globals`
   name (`last_nil_ref_global`). The immediately-following nil-op error consults that stamp for a
   precise attribution; if the stamp doesn't match, fall back to a general note listing `ref_globals`
   ("one of these globals is a dropped reference and nil here"). This is best-effort naming layered on
   top of the always-correct provenance tag.

## Failure modes

Creation-time (fast, explicit):
- escaping upvalue is a reference → `EscapingReference(slot)`
- constant is a non-function reference (e.g. embedded native fn) → `NonConstantConstant`

Run-time (Lua-normal, but annotated):
- dropped reference used as nil → normal nil error **plus** the provenance tag and (when traceable)
  the dropped-reference amendment above.

## Staging

1. **Pure snippets — DONE** (`src/snippet.rs`, `Lua::extract_snippet`/`build_snippet`/`call_snippet`/
   `run_snippet`, `tests/snippets.rs`). Proto tree copied cross-arena, globals classified (value →
   snapshot, reference → resolve from the target VM), materialized + run in a fresh isolated VM.
   `build_snippet` returns the built VM as a standalone handle for repeated `call_snippet`. Upvalue
   capture is rejected for now (`UpvaluesNotYetSupported`).
2. **Escaping value upvalues** — snapshot + closed-upvalue rebuild.
3. **Global snapshot** (option A) + `external_globals` resolution against B.
4. **Nested functions** inside the snippet (recursive proto copy; internal upvalues at run time).
5. **Surface** — a Lua `snippet(fn)` builtin returning an opaque handle, plus a Rust embedding API
   (`Lua::extract_snippet` / `Lua::run_snippet`), and `ExVal` result marshaling.

## Open questions

- **API shape:** Lua builtin `snippet(fn)` → opaque handle, Rust `Lua::extract_snippet`/`run_snippet`,
  or both? (I lean both — the builtin for scripts, the Rust API for the host/engine.)
- **Handle representation in Lua:** a new `Value` variant (`Snippet`) vs. userdata wrapping the plain
  `Snippet`? Userdata avoids touching the 32-byte `Value` budget and the many exhaustive matches.
- **Where does B live?** Engine-managed pool of sandbox VMs vs. one-per-snippet. (Petrichor question.)
```
