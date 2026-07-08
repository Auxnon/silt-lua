# Silt-Lua — Static Typing Plan (Luau-style, gradual, compile-time)

> Companion to `PLAN.md` §3.5. This is the detailed roadmap for optional static
> types that error at **compile time**. Goal: Luau-flavored gradual typing —
> annotate where you want safety, infer/ignore elsewhere — without a runtime
> cost (types never reach the VM) and without abandoning silt's single-pass,
> AST-free compiler.

---

## 0. The core tension

Silt's compiler is **single-pass and AST-free**: it Pratt-parses tokens and
emits bytecode in one sweep. Textbook type checkers do the opposite — build an
AST, then run one or more separate passes over it. That mismatch shapes every
decision here.

Two consequences:

1. **No forward references for free.** When the single pass reaches a use of a
   type `Widget` (or calls a function `f`) before its definition has been
   parsed, the definition simply isn't known yet. A pure single pass can only
   check facts already in scope.
2. **Checking must be woven into emit, or bolted on as extra passes.** We choose
   a hybrid: a cheap *declaration pre-scan* up front, then checking woven into
   the existing emit via a "type stack."

### What "define types ahead of time" really means

A common intuition is that languages require type definitions at the top of the
file. They don't (C's forward declarations aside). What actually happens in
TypeScript, Rust, Go, C#, Luau, … is a **declaration-gathering pass**: the
compiler walks the source once collecting all named types / signatures into a
*type environment* before it checks any expression. Order doesn't matter because
the environment is fully populated before checking begins. Our pre-scan (Phase
1.5) is exactly this gathering pass, scoped down to what a single-pass VM needs.

---

## 1. How Luau does it (reference)

- **Full AST + a type pass separate from codegen.** Parse → infer/check → lower.
  Three stages, not one.
- **Type aliases hoisted per scope.** `type A = …` is usable before its textual
  definition within its block, and aliases may be **mutually recursive**
  (`type Tree = { kids: {Tree} }`). Enabled by gathering aliases in a scope
  before resolving them.
- **Inference, not just declared types.** `local x = 5` ⇒ `x: number` with no
  annotation; types propagate through expressions (bidirectional / local
  inference; the newer solver does more global inference). You annotate the
  boundaries; inference fills the interior.
- **Gradual, two modes.** `any` is compatible with everything. Nonstrict is
  lenient; strict complains. Errors are diagnostics, not hard stops (until you
  opt into strict).
- **Flow-sensitive refinement (narrowing).** `if type(x) == "string" then …`
  narrows `x` to `string` in that branch.
- **Rich types:** generics, unions `A | B`, intersections `A & B`, optionals
  `T?`, singleton/literal types, structural table/record types.

We approximate the useful 80% of this inside a single-pass VM.

---

## 2. Architecture decision

Three ways to bridge the single-pass gap:

| Option | What | Cost | Verdict |
|---|---|---|---|
| **A. Declaration pre-scan ("1.5 pass")** | A cheap first scan collects `type` aliases + top-level fn signatures; main pass checks against it | Low–medium; no AST | **Chosen** |
| B. Full AST + type pass | Parse to AST, real inference pass, then lower | High; abandons single-pass codegen | Deferred / maybe never |
| C. Strict single-pass only | Only check what's already in scope; forward refs = `any` | Lowest; least complete | This is Phase 1 today |

**Chosen: A.** It gives forward references and mutually-recursive aliases (the
behavior the "define ahead of time" intuition wants) while keeping codegen
single-pass. Full inference and narrowing layer on later.

---

## 3. The type representation

Phase 1 (shipped, `src/types.rs`, gated `#[cfg(feature = "typing")]`):

```rust
enum Type {
    Any,                    // dynamic/unknown — compatible with everything
    Nil, Boolean, Number, String,
    Table,                  // opaque for now
    Function,               // opaque for now
    Optional(Box<Type>),    // T?  (variant exists; `?` not lexed yet)
    Named(String),          // unresolved alias — treated as Any for now
}
```

`Type::from_name` maps annotation idents; `assignable(target, value)` is the
single compatibility chokepoint (gradual: `Any` / `Named` compatible with all).

Planned growth:
- `Union(Vec<Type>)`, `Intersection(Vec<Type>)`
- `TableShape { fields: Map<String,Type>, array: Option<Box<Type>>, … }`
- `FnSig { params: Vec<Type>, variadic: Option<Box<Type>>, returns: Vec<Type> }`
- `Generic(name)` + instantiation
- resolve `Named` against the alias environment (Phase 1.5)

---

## 4. Phases

### Phase 1 — track, don't check  ✅ DONE
- `Type` lattice + `assignable` + `Local.ty`.
- Parse & store annotations: `local x: T`, `local a: T, b: U`, `function f(a: T)`,
  arrow params `(a: T) -> …`. Annotations currently parsed-and-(mostly)-stored;
  no checking. Zero impact with the feature off.
- Tests: `tests/typing.rs`.

### Phase 1.5 — declaration pre-scan (the key enabler)
- Before the main compile pass (or as a lightweight token pre-walk), scan for:
  - `type Name = <texpr>` aliases (and `export type` if we add modules)
  - top-level `function`/`local function` **signatures** (params + return types)
- Build a **type environment**: `aliases: Map<String, Type>`, `fn_sigs: Map<…>`.
- Resolve `Named` lazily/iteratively to support **forward references** and
  **mutual recursion** (`type A = {b: B}` / `type B = {a: A}`): collect all
  names first, then resolve bodies, leaving cycles as references.
- Implementation note: the pre-scan must NOT emit bytecode — it only reads
  declarations. Either a second `Lexer` pass over the source, or a recorded
  token buffer. Skipping expression bodies keeps it cheap.
- Output feeds Phases 2–3 so a use of `Widget`/`f` before its definition checks
  correctly instead of degrading to `Any`.

### Phase 2 — cheap, high-confidence checks (type stack)
- Maintain a **type stack mirroring the operand stack**. Wherever emit pushes a
  runtime value, push its static `Type`; operators pop operand types, check
  compatibility, push the result type.
  - literals → their type; locals → `Local.ty`; globals/unknown → `Any`.
  - `a + b` ⇒ both `Number`-assignable else error; `..` ⇒ string-coercible;
    comparisons ⇒ `Boolean`.
  - call of a known-non-`Function` → error; index of a known-non-`Table` → error.
- Annotated-assignment mismatch: `local n: number = "x"` → error.
- Only flag when **both** sides are concretely known and incompatible (gradual).
- Diagnostics collected into the existing `Compiler.errors`; non-fatal by default.

### Phase 3 — function signatures + arg/return checking
- Record `FnSig` per function (params from annotations, returns from `): T` —
  note the `)`-absorption quirk in `build_function` must be handled to capture
  return types; see PLAN §3.5/§3.6).
- Check call sites: arg arity + per-arg `assignable`; the call expression's
  result type comes from the signature's returns.
- Uses the Phase-1.5 environment so forward / mutually-recursive calls check.

### Phase 4 — rich types + narrowing + strict mode
- `T?` optionals (needs lexer `?`), `A | B` unions, `A & B` intersections,
  structural table shapes `{x: number, …}`, generics.
- **Flow narrowing:** refine types inside `if type(x) == "…"` / `if x` branches.
- **`--!strict`** file flag (reuse the lexer's `--!` flag mechanism) flips
  diagnostics from warnings to hard errors; default stays nonstrict/gradual.

---

## 5. Cross-cutting decisions

- **Gradual is load-bearing.** `Any` (and unresolved `Named` until Phase 1.5)
  compatible both ways. The checker must be quiet on the huge dynamic surface or
  it's worse than useless. Error only on provable incompatibility.
- **Compile-time only.** No `Type` ever reaches the VM; bytecode is unchanged.
  (A *later* payoff: known types could let the compiler emit specialized
  arithmetic opcodes — ties into the §6 register-VM/perf goals — but that's far
  out and orthogonal to checking.)
- **Feature-gated.** Everything behind `#[cfg(feature = "typing")]`; off by
  default; `LanguageFlags` field gated like `bang`/`compound_assignment`.
- **Retire the dead lexer path.** `ColonIdentifier`/`Typer`/`colon_blow` are
  unused; when typing matures, delete them so the colon has one model
  (`Token::Colon`), shared by method calls and annotations.
- **The `(a: …` ambiguity** (typed arrow param vs parenthesized method call) is
  already resolved in `grouping_or_arrow` + `finish_grouped_method_call`; keep
  that behavior as annotations grow.

---

## 6. Open questions

- Do we want module-level `export type` / cross-file types, or single-file only?
  (Affects whether the pre-scan is per-file or whole-program.)
- Integer vs number: keep unified `number` (Luau-like, current) or expose an
  `integer` subtype? Unified avoids a class of false positives.
- How much inference in Phase 2 — just literal initializers, or propagate
  through simple expressions? Start minimal (literals + annotated locals).
- Error UX: dedicated `SiltError::TypeError { expected, found, loc }` with good
  messages; how to surface non-fatal diagnostics in the CLI.
