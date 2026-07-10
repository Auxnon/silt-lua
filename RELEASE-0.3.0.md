# silt-lua 0.3.0

A large release covering everything since **0.1.1**: new language features, a real
standard library, working metatables/metamethods, a language server, hot-swapping, a
glam-backed vector type, and a broad correctness + performance pass. Semantics target a
defensible subset of Lua 5.3/5.4 (coroutines still out of scope).

## Highlights

- **Vectors** — first-class 2D/3D/4D vector value type backed by [glam] (f32, immutable),
  behind the `vector` feature.
- **Working metatables & userdata metamethods** — table-valued `__index`, arithmetic,
  comparison, `__concat`, `__tostring`, `__pairs`; userdata methods and fields dispatch.
- **Language server** — pure-Rust analysis core plus a JSON-RPC stdio server (`silt lsp`)
  for Neovim et al., behind `lsp`/`lsp-server`.
- **Hot-swapping** — live global and nested/instance method replacement in a running VM,
  behind `hot-swap`.
- **Native multi-return ABI** — generic-for with `pairs`/`ipairs`/`next`, and an
  mlua-style Rust return convention (tuples spread, collections → tables).
- **Arrow functions**, **compound assignment**, **bitwise operators**, and **varargs**.

## Language features

- **Arrow functions** (`arrow`, on by default): `x -> x + 1`, `(a, b) -> a + b`, with a
  `do … end` body; always implicit-returns its last expression.
- **Varargs & multiple returns**: `...` parameters, multi-value returns, and multi-target
  assignment (`local a, b = f()`), with extra targets padded to `nil`. A trailing call in
  call position spreads its results (`f(g())`, `f(unpack(t))`).
- **Compound assignment** (`compound-assignment`): `+= -= *= /= //= %= ^= ..=`.
- **Bitwise operators**: `& | ~ << >>` (and unary `~`).
- **Control flow**: `repeat … until`, proper `break`, and descending numeric `for`
  (negative step). Generic `for … in` with `pairs`/`ipairs`/`next`.
- **Functions as expressions/statements**: assign functions to variables, declare inline.
- **Static typing** (`typing`, off by default): Phase 1 parses and tracks Luau-style
  annotations (no checking yet).
- **Field/index access on grouped & call results**: `(a + b).x`, `(t)["k"]`, `f().field`.

## Standard library

- **Base**: `type`, `tostring` (Lua-style `table: 0xADDR` identity for reference types),
  `tonumber` (with base arg), `assert`, `error`, `pcall` (protected calls that catch
  errors instead of crashing the VM), `next`, `pairs`, `ipairs`.
- **`table`**: `insert` (append + positional with shift-up), `remove` (last + positional),
  `concat`, `unpack`.
- **`math`**: `floor`, `ceil`, `abs`, `sqrt`, `sin`/`cos`/`tan`, `min`, `max`, `random`,
  `randomseed`, `modf`, `huge`, `pi`, `maxinteger`, `mininteger`.
- **`string`**: `len`, `sub`, `upper`, `lower`, `rep`, `reverse`, `byte`, `char`, `format`,
  plus method-call sugar (`s:upper()`) via the string metatable, and decoded escape
  sequences. String delimiters are no longer included in the value.

## Metatables & userdata

- **Metatables**: `setmetatable`/`getmetatable`, table-valued `__index` (OOP works),
  arithmetic metamethods (`__add`/`__sub`/…), comparison (`__eq`/`__lt`/`__le`), `__concat`,
  reference equality.
- **Userdata metamethod dispatch** was un-stubbed: arithmetic, `__concat`, `__tostring`,
  `__eq`/`__lt`/`__le`, and **`__pairs`** now fire on userdata; methods (`ud:m()`) and
  field get/set work. `pairs(userdata)` iterates via `__pairs`.
- **Entity interop**: userdata fields can be typed as vectors (`glam::Vec3`), so
  `entity.pos = entity.pos + vec3(0, 10, 0)` works with no coercion metamethod.

## Vectors (`vector` feature)

Immutable f32 `Vec2`/`Vec3`/`Vec4` backed by glam.

- Constructors `vec2/vec3/vec4(...)`; fields `.x/.y/.z/.w`; `type(v)` → `"vec2"/"vec3"/"vec4"`.
- Operators `+ - * /` (component-wise **and** scalar broadcast in either order), unary `-`,
  `==`.
- Methods `:length() :length_squared() :dot() :normalize() :distance() :cross()`.
- `FromLua`/`ToLua` for the wrappers and raw `glam::Vec*`; glam is re-exported as
  `silt_lua::glam`.

## Embedding / Rust API

- **mlua-style multi-return**: native functions bind `R: ToLuaMulti` — a **tuple** spreads
  to multiple Lua values, a **collection** (`Vec`, `[T; N]`) becomes one **table**, a scalar
  is one value. `FromLua`/`ToLua` widened accordingly.
- **Userdata**: registration of methods, metamethods, and field getters/setters; weak
  references that upgrade; ref vs. mut method variants; many value-cast conversions
  (arrays, optionals, primitives) for arguments and returns.
- **Errors**: runtime errors bubble up with line/column numbers and a source-snippet
  helper (`error_snippet`); a `source_index` tracks which script raised an error.

## Tooling

- **LSP** (`lsp` / `lsp-server`): a pure-Rust analysis API (diagnostics + formatting, no
  serde) and a JSON-RPC-over-stdio server launched via `silt lsp`.
- **Hot-swap** (`hot-swap`): live global swap (stage 1) and nested/instance method swap via
  prototype redirect (stage 2); incremental GC each VM cycle with a full collection after a
  swap.
- **CLI**: run a Lua string directly (`silt --run "…"`), a file path, or `--help`.

## Performance

- Release profile tuned for speed; hot helpers inlined; adjacent stack pops merged.
- Numeric-for loop control fused into a single tail `FORLOOP` opcode.
- Assignment setters consume their value (dropped a trailing `POP` and a clone).
- Native-call arguments reuse a scratch buffer — no per-call `Vec` allocation
  (~18% faster on native-call-heavy code).
- Hot-swap redirect cell gated behind the feature so the default build pays nothing.

## Correctness & compiler hardening

- **Compiler leniency tightened**: stray tokens that can't begin an expression are now
  reported (`local x = )`, `return )`, `end`, `x = = 5`, …); `local`/`global` with no
  identifier errors instead of panicking (which could take down the LSP).
- **Fixes**: `elseif` parsing; grouping operator-order; string comparison + escapes;
  `if … end` edge cases; table-set that didn't set; method-call chaining with `self`;
  upvalue/off-by-one issues from the vararg work; `for`-loop params treated as single
  expressions (not multi-value); a non-literal numeric-`for` start (`for i = #a, 5`) whose
  separator comma was swallowed by multi-assign detection; a bare uninitialized `local x`
  (no `=`) that left its slot unfilled and mis-aligned every later local (and could panic a
  later closure capture with "attempt to subtract with overflow"); vararg offsets and trailing
  values.
- **Embedding fixes**: `u64`/`usize` ↔ `Value` conversions used swapped/overflowing bounds;
  f32 clamping; userdata colon self-calls `ud:method(param)`; `UserDataWrapper` bounds; a
  table getter/setter that leaked `self` onto the stack; `call_fn`/`call_with_params` now
  deliver runtime args to a loaded chunk as `...`.
- **Lexer**: tracked char counts but byte-sliced the source (multibyte-safe now).
- **wasm** build compatibility corrections.

## Feature flags

Default bundle (`silt`): `bang`, `under-number`, `global`, `implicit-return`,
`short-declare`, `compound-assignment`, `arrow`.

Opt-in: `typing`, `hot-swap`, `lsp`, `lsp-server`, `vector`, `dev-out`.

[glam]: https://docs.rs/glam
