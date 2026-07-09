# Silt-Lua — Conformance & Roadmap Plan

> **Audience:** future AI agents and contributors working on the interpreter.
> **Goal:** bring silt-lua to a defensible subset of Lua 5.3/5.4 semantics (coroutines
> explicitly out of scope for now), with a regression test suite that encodes the target
> behavior and a benchmark harness for tracking performance against PUC-Lua.
>
> This document was produced by auditing the interpreter against a broad battery of Lua
> language scenarios (see `tests/`). Every claim below was reproduced against the code on
> branch `copilot/sub-pr-10-again`. Each item lists a **repro**, the **observed** behavior,
> the **expected** Lua behavior, and a **root-cause pointer** (`file:line`) plus a fix sketch.

---

## 0. How to work on this

- **Run the suite:** `cargo test` (uses the default `silt` feature set).
- **Status convention in tests:** features that work have live, asserted tests. Features that
  are broken or unimplemented have tests marked `#[ignore = "PLAN.md §X — <reason>"]`. They
  encode the *correct* expected behavior so that fixing a feature is a matter of removing the
  `#[ignore]` and watching it pass. Run them with `cargo test -- --ignored`.
- **Integer vs Number:** Lua 5.3+ has an integer subtype. `2 + 3` is `Integer(5)`, `2.5 + 1`
  is `Number(3.5)`, and `/` and `^` always produce floats. The interpreter already does this
  correctly; the *original* test suite was authored expecting `Number` everywhere and was
  wrong. Tests have been corrected to assert `Integer` where Lua produces an integer.
- **No CI config exists yet** (`.github/workflows` is absent). See §7 for the proposed CI job
  and how the benchmark batch is kept out of it.

---

## 1. Critical bugs (crashes / hangs) — fix first

These take the interpreter down hard (Rust `panic!`, index-out-of-bounds, or an infinite
loop). They must be fixed before the suite can be trusted, because they can abort a whole
test binary.

### 1.0 Compiler is far too lenient — silently accepts malformed source 🟢 LARGELY FIXED (no-prefix + missing-ident classes done; only an optional Lua-parity check parked)
- **Repro (all compile + "succeed" with NO error):** `local x = )`, `local = 5`, `x = = 5`,
  `return )`, `)`, `end`, `1 2 3`. The single-pass compiler emits whatever bytecode it can
  and moves on instead of reporting a syntax error.
- **Why it matters now:** the new LSP (`src/lsp.rs`, feature `lsp`/`lsp-server`) surfaces
  this directly — diagnostics can only report what the compiler actually flags, so editors
  see a near-empty error set on obviously-broken code. It also masks real user mistakes at
  runtime (garbage executes instead of erroring).
- **What works today:** only a handful of cases are caught — unterminated blocks
  (`if x then` → "Unterminated block"), missing tokens (`function` → "Expected token (").
- **Direction:** tighten the parser/`expression`/`statement` paths to *expect* a valid
  token and push a `SiltError` (then `synchronize`) on anything unexpected, rather than
  silently accepting. Add a `tests/errors.rs` matrix of must-error inputs. This is the
  highest-leverage correctness + tooling investment after the current feature work.
- **Progress (2026-07, `tests/stray_tokens.rs`):** the *no-prefix-token* class is now
  caught. `parse_precedence` used to call the `void` prefix rule (a no-op) for any token
  that cannot begin an expression — a stray `)`, a leading/dangling binary operator, `end`,
  a second `=` — silently dropping it. It now reports `InvalidTokenPlacement` instead, so
  the CLI and LSP flag them (verified in-editor: a broken paren pair now diagnoses).
  Now-caught: `local x = )`, `return )`, `)`, `end`, `x = = 5`, `local x = 1 +`, `return * 2`,
  `return (1 + 2))`. Also the *missing-identifier* class: `local`/`global` not followed by
  an identifier or `function` (`local = 5`, `local 5`, `global = 5`, bare `local`) used to
  `todo!()` — a **panic** that would take down the LSP server — and now reports
  `ExpectedLocalIdentifier` pointing at the offending token. Fixing that also surfaced that
  `push_error` (compile-loop error sink) never set `valid = false`, so any error built as a
  raw `ErrorTuple` (e.g. `peek_triple`'s EOF branch) was recorded but the chunk still
  "succeeded" and ran garbage; `push_error` now invalidates. Turning on the no-prefix
  error exposed a latent bug: `build_function` never consumed the `)` closing its parameter
  list — it leaned on the body's first (phantom) statement to swallow it, which *also*
  emitted the placeholder leading POP the call convention skips and cleared
  `local_declare_mode`. `build_function` now does all three explicitly (see `src/compiler.rs`
  around the param loop). Without the last two, `local function f(a,b) …` misresolved every
  param slot and function bodies dropped their first instruction.
- **Deferred / optional (backburner) — juxtaposed values are NOT leniency:** `1 2 3` → `3`
  and `function f() 1 2 3 end` → `3` are the *implicit-return feature working as designed*,
  not a bug. Bare expression statements are load-bearing: silt emits a `POP` after each and
  drops the final one at block end so the last value survives (Lua proper forbids bare-value
  statements, which is why it errors on `1 2 3`; silt deliberately relaxed that). So the "add
  an expected-separator check" idea from earlier is wrong — it would reject valid implicit
  returns. If we ever want to catch the dead-value typo, the only safe rule is narrow: *a
  bare, non-call expression statement is legal only as the **last** statement of an
  implicit-return block* (arrow bodies always; regular functions only when `implicit-return`
  is on) — flag the non-last bare values (the ones that get computed then immediately popped).
  This keeps `f() g() 42`, `local a=10 a`, and every arrow case, while catching `1 2 3` via
  its discarded `1`/`2`. Cost: track each expr-statement's kind + position and flag non-last
  bare ones at block close (single-pass ⇒ "is last" only known at `end`). Low payoff (dead
  value, no crash/corruption), real behavior change — parked unless we want the Lua-parity.

### 1.1 Closure / upvalue capture panics — `index out of bounds` ✅ FIXED
- **Repro:**
  ```lua
  local function mk() local c = 0 return function() c = c + 1 return c end end
  local f = mk(); f(); return f()        -- expect 2
  ```
  Also: any for-loop closure capture (`for i=1,3 do a[i] = function() return i end end`) and
  the existing lib tests `closures`, `closures2`, `scope`.
- **Observed:** `panic!` — `index out of bounds: the len is N but the index is N` at
  `src/compiler.rs:1529` (and the sibling indexing at `src/compiler.rs:1508`,
  `this.local_functional_offset[this.functional_depth - 1]`).
- **Expected:** closures capture enclosing locals as upvalues; counter/accumulator patterns work.
- **Root cause:** `resolve_upvalue` (`src/compiler.rs:1522`) indexes `functional_states[level]`
  and `local_functional_offset[functional_depth - 1]` without bounds reconciliation. The
  vararg work on this branch (`local_functional_offset`, commits `f44fc4f`→`60257ee`) shifted
  local indices but did not keep the upvalue resolver's `level`/`offset` math in range.
- **Fix sketch:** audit the invariant between `functional_depth`, `local_functional_offset`,
  and the `functional_states` stack. Add a debug assertion at the top of `resolve_upvalue`
  and a focused unit test for two- and three-level capture. This is the single highest-value
  fix — closures are a headline feature and are currently fully broken.
- **Resolution (2026-06):** two bugs. (a) `resolve_upvalue` indexed `functional_states[level]`
  by functional *depth*, but that vec excludes the root frame (root uses `root_state`), so the
  current function's state lives at `level - 1`; indexing by `level` ran one past the end →
  the panic. (b) Even after the resolver was fixed, `close_upvalues_by_return`
  (`src/lua.rs`) read a *bitwise copy* of each `UpValue` out of its `Gc` cell and called
  `close()` on the copy, so the real heap cell stayed open pointing at the returning frame's
  dead stack slot; it also never drained closed upvalues. Rewrote it to `borrow_mut` the actual
  cell, close it, and remove every upvalue at/above the frame base. `tests/closures.rs`:
  `counter_closure`, `for_loop_capture`, `nested_capture`, `multi_level_upvalue` now pass
  (global- and local-function forms both work). `method_definition_colon` remains ignored — it
  is §1.2 (colon-method parser `todo!`), unrelated to upvalues.

### 1.2 Method/field function definition panics ✅ FIXED
- **Repro:** `local t = {x=5}; function t:get() return self.x end` — also `function t.get() ... end`.
- **Observed:** Rust panic (same upvalue-resolution path as 1.1).
- **Expected:** defines `t.get` with implicit `self` (colon form).
- **Root cause:** the `function <expr>:<name>()` / `function <expr>.<name>()` path was not parsed
  at all — `define_function` expected `(` straight after the name and otherwise the old `typing`
  path hit a `todo!`. (The 1.1 panic was a red herring; this is purely a parser gap.)
- **Resolution (2026-06):** added `define_function_member` (`src/compiler.rs`). It loads the base
  receiver, walks intermediate `.field`s with `TABLE_GET`, builds the closure, and `TABLE_SET`s it
  into the final key. `build_function` gained an `is_method` flag that injects an implicit `self`
  local (slot 0) for the colon form, matching the receiver the call site pushes as arg 0. Emitted
  code is stack-neutral so no statement pop is needed. Tests: `method_definition_colon`,
  `method_definition_colon_with_args`, `dot_function_definition` pass.

### 1.2.1 Colon-method *calls* on a chained receiver ✅ FIXED
- **Repro:** `local a = {b={x=100}}; function a.b:add(n) return self.x+n end; return a.b:add(5)` → `105`.
  Also `a.b.c:get()`.
- **Observed:** the receiver was returned unchanged / `self` or args were wrong — the
  `Token::Colon` method-call path only re-emitted a *single* getter as `self` (`pull_getter`),
  so a chained receiver like `a.b` re-evaluated to just `a`, and the `Dot`/`Bracket` branch
  consumed the chain and returned before the trailing `:m(...)` was parsed at all.
- **Resolution (2026-06):** added a `METHOD_GET { constant }` opcode (mirrors PUC-Lua `OP_SELF`).
  It takes the fully-evaluated receiver on the stack top and a method-name constant, looks up
  `receiver[name]`, and leaves `[method, receiver]` so the receiver becomes the implicit `self`.
  Both colon-call branches now funnel through `emit_method_get` — the bare-variable branch drains
  its getter, the chained branch emits `TABLE_GET` first — and neither has to recompute the
  receiver. Removed the now-dead `pull_getter`. Tests: `method_call_on_chained_receiver`,
  `method_call_deep_chain`.

### 1.2.2 Plain `if … then … end` corrupts a live local ✅ FIXED
- **Repro:** `local x = 5 if x == 0 then x = 1 end return x + 100` → `Cannot + 'nil' and 'integer'`.
  Also any `local function m(n) if n == 0 then return -1 end return n + 1 end`. Discovered while
  finishing §1.2.1 — method bodies with an `if … return … end` guard were the only ones failing.
- **Observed:** a local declared *before* a plain `if` reads as `nil` after the if-block. (Not
  specific to early `return` or to functions — the trigger is simply having a live local on the
  stack across a plain `if`.)
- **Root cause:** a plain `if … then … end` (the no-`else` arm of `if_statement`,
  `src/compiler.rs`) never consumed its own `end`. `build_block_until!` stops *at* `end` without
  eating it; the `Else` arm eats it via `expect_token!` and the `ElseIf` arm recurses, but the
  bare arm only patched the jump. The leftover `end` was then handed to `expression_statement`,
  which compiled it as an empty expression and emitted a stray `OP_POP` — popping the slot of the
  local sitting below — before the `end` was swallowed. Earlier if-tests passed only because they
  read no local afterward, so the stray POP was invisible.
- **Resolution (2026-06):** the bare arm now does `expect_token!(this it End)`. One-line fix; kills
  both the leaked `end` and the stray POP. Tests: `plain_if_preserves_prior_local`,
  `function_if_early_return_keeps_params`, `plain_if_no_semicolon_then_statement`
  (`tests/conditionals.rs`).

### 1.3 Bitwise operators hang the compiler ✅ FIXED
- **Repro:** `return 6 & 3` (also `|`). The process spins forever.
- **Observed:** infinite loop; no error, no result.
- **Expected:** Lua 5.3 integer bitwise: `6 & 3 == 2`, `4 | 1 == 5`, `5 ~ 1 == 4`,
  `~0 == -1`, `1 << 4 == 16`, `256 >> 2 == 64`.
- **Root cause:** two issues. (1) `synchronize()` was a **no-op**, so when `compile()`'s
  `while iter.peek().is_some()` loop hit a `declaration` error that left the offending token in
  place (e.g. an unlexable char surfacing as a peek `Err`), it re-parsed that token forever.
  (2) `&`, `|`, `<<`, `>>` were not lexed and binary `~` had no infix rule.
- **Resolution (2026-06):**
  - (a) `synchronize(iter)` now always consumes at least one token and skips to the next
    statement boundary — kills the hang class for *any* stray character, not just `&`/`|`.
  - (b) lexer emits `&`/`|` (`BitAnd`/`BitOr`), `<<`/`>>` (`ShiftLeft`/`ShiftRight`); `~` is
    unary not (prefix) and binary xor (infix), reusing `Operator::Tilde`.
  - (c) added `BIT_AND`/`BIT_OR`/`BIT_XOR`/`BIT_NOT`/`SHIFT_LEFT`/`SHIFT_RIGHT` opcodes and the
    Lua 5.3 precedence ladder (`|` < `~` < `&` < `<< >>`, all between comparison and concat).
    VM coerces operands to integers (float with exact integer value ok; else
    `ExpInvalidBitwise`); shifts are logical 64-bit with Lua's out-of-range/negative semantics.
  - Tests: `bitwise`, `bitwise_precedence` (`tests/arithmetic.rs`), `bitwise_on_non_integer_errors`
    (`tests/errors.rs`). `cargo test -- --ignored` no longer hangs.
  - **Not done (separate):** table `__band`/etc metamethods aren't dispatched (tables error); hex
    integer literals (`0xff`) are unrelated and still unsupported by the lexer.

---

## 2. Major correctness bugs

### 2.1 Arithmetic operators with no opcode: `%`, `^`, `//` ✅ FIXED (bitwise still pending §1.3)
- **Repro:** `return 5 % 2` → `5`; `return 17 % 5` → `17`; `return 2 ^ 10` → `2`;
  `return 7 // 2` → error/garbage.
- **Observed:** the operator is silently dropped; the result is the **left operand**.
- **Expected:** `5 % 2 == 1`, `5.5 % 2 == 1.5` (floored modulo), `2 ^ 10 == 1024.0`
  (`^` always float), `7 // 2 == 3` (floor division).
- **Root cause:** `%` lexed to `Operator::Modulus` but had no Pratt rule/opcode; `^` and `//`
  were not lexed at all. With no infix rule the precedence loop just stopped and trailing tokens
  were abandoned.
- **Resolution (2026-06):** lexer now emits `^` (`Operator::Exponent`) and `//`
  (`Operator::FloorDivide`); added `MODULUS`/`POWER`/`FLOOR_DIVIDE` opcodes (`src/code.rs`);
  Pratt rules — `%` and `//` at `Factor`, and a new `Exponent` precedence level above `Unary`
  for `^` with a dedicated right-associative `exponent` infix (so `2^2^3 == 256` and
  `-2^2 == -4`). VM handlers implement Lua coercion: `%` floored (sign of divisor, integer-by-0
  errors instead of panicking), `^` always float, `//` floored toward -inf; tables dispatch the
  `Mod`/`Pow`/`IDiv` metamethods. Tests: `modulo`, `power`, `floor_division`,
  `arithmetic_precedence` (`tests/arithmetic.rs`).
- **Still pending:** bitwise `&` `|` `~` `<<` `>>` remain unlexed and are blocked on §1.3
  (`&`/`|` hang the parser). `bitwise` test stays ignored.

### 2.1b Compound assignment (Luau-style `+= -= *= /= //= %= ^= ..=`) ✅ NEW FEATURE
- New cargo feature `compound-assignment` (in the default `silt` set) plus a runtime flag
  `LanguageFlags::compound_assignment` (defaults from the feature). The lexer only emits the
  compound tokens when the feature is on (otherwise `+=` falls back to `+` then `=`).
- `x op= e` desugars to `x = x op e`, evaluating the target **once**. Simple variables
  (local/upvalue/global) emit getter → RHS → op → setter. Table targets (`t.f op= e`,
  `t[k] op= e`, chained `a.b.c op= e`) use a new `DUP_N(n)` opcode to duplicate the
  receiver+keys so the same operands feed a `TABLE_GET` and a `TABLE_SET` with no
  re-evaluation. Multiple targets (`a, b += 1`) are rejected. `^=` (PowerAssign) maps to the
  §2.1 `POWER` opcode. Tests: `tests/compound_assign.rs`.

### 2.1c `LanguageFlags` fields are feature-gated ✅
- `LanguageFlags` fields now only exist when their backing cargo feature is compiled in:
  `bang_operator` is behind `#[cfg(feature = "bang")]` and `compound_assignment` behind
  `#[cfg(feature = "compound-assignment")]`, with `Default`, `new_with_flags`, and every read
  (the two compound-assign match arms + the `compound_op`/`compound_assign_var` helpers) under
  the same `#[cfg]`. A disabled feature leaves no dead config surface. (`implicit_returns` and
  `arrow_functions` are left ungated — the former is always-on core behavior, the latter has no
  feature flag yet.) Verified compiling under `--no-default-features` and each feature alone.

### 2.2 Parenthesized sub-expression drops trailing operators 🔴
- **Repro:** `return (1+2)*3` → `3`; `return (10+5)*2-3` → `15`; `return (1+2)+3` → `3`.
  (Note: `2*(1+2)` → `6` works, because the paren group is not followed by another operator.)
- **Observed:** any operator *after* a closing `)` is dropped; the value is just the group.
- **Expected:** `(1+2)*3 == 9`, `(10+5)*2-3 == 27`.
- **Root cause:** `grouping()` (`src/compiler.rs`) calls `expression()` but **never
  consumes the closing `)`** — the `expect CloseParen` was commented out. The
  stray `)` then sits at the cursor with no infix rule, so the enclosing precedence loop halts
  and the following operator is never parsed.
- **Resolution (2026-06) ✅ FIXED:** `grouping()` now `expect_token!`s `CloseParen` at the end,
  erroring with `UnterminatedParenthesis(line,col)` (col captured from the open paren) when it is
  missing. One small fix, large blast radius. Tests: `parentheses_then_operator` (extended with
  nested groups `((1+2)*(3+4))`, `2*(3+(4-1))`).

### 2.3 `elseif` fails to parse ✅ FIXED
- **Repro:** `local x=2; if x==1 then return 10 elseif x==2 then return 20 else return 30 end`
- **Observed:** `Expected token: then`.
- **Expected:** returns `20`.
- **Root cause:** two bugs in the `ElseIf` arm of `if_statement` (`src/compiler.rs`). (a) It ate
  the `ElseIf` token and *then* recursed into `if_statement`, whose own leading `eat()` consumed
  the first token of the elseif condition — so `elseif x==2 then` lost `x` and the parser hit
  `==` where it wanted `then`. (b) It emitted no forward jump over the chain, so a taken `if`
  branch would fall through and also execute the elseif/else bodies.
- **Resolution (2026-06):** the `ElseIf` arm now mirrors the `Else` arm: emit a `FORWARD(0)`
  (`skip_chain`) after the block, patch `skip_if` to the elseif condition, then recurse WITHOUT
  eating `ElseIf` (the recursion's leading `eat()` consumes it, just like an `if`), and finally
  patch `skip_chain` to just past the whole chain (the recursion eats the single closing `end`).
  Tests: `if_elseif_else`, `if_elseif_chain_and_fallthrough` (`tests/conditionals.rs`); that file
  now has zero ignored tests.

### 2.4 String relational comparison unimplemented ✅ FIXED
- **Repro:** `return 'abc' < 'abd'` → `Cannot < 'string' and 'string'`.
- **Expected:** lexicographic: `'abc' < 'abd' == true`, `'b' > 'a' == true`, `'abc' <= 'abc' == true`.
- **Root cause:** the `LESS`/`LESS_EQUAL`/`GREATER`/`GREATER_EQUAL` opcode handlers in the VM
  (`src/lua.rs`) only accept numeric operands. (`==`/`~=` on strings already work.)
- **Resolution (2026-06):** added `(String, String)` arms to `is_less`/`is_greater` comparing by
  Rust `str` `Ord` — byte order, matching Lua's default-locale `strcmp`. `<=`/`>=` reuse these via
  the existing negation in the opcode handlers. Mixed string/number still type-errors per Lua.
  Tests: `string_comparison` (`tests/strings.rs`).

### 2.5 String escape sequences not decoded ✅ FIXED (bare-minimum set)
- **Repro:** `return 'a\nb'` → the literal three chars `a`, `\`, `n`, `b` (backslash kept).
- **Expected:** `\n`, `\t`, `\\`, `\"`, `\'`, `\r`, `\0`, `\xHH`, `\ddd`, `\u{XXXX}`, `\z` decoded.
- **Root cause:** `Lexer::string()` copied the raw slice without an escape-decoding pass.
- **Resolution (2026-06):** `Lexer::string()` now accumulates decoded characters and handles the
  single-character escapes `\n \t \r \\ \" \' \0 \a \b \f \v`; an escaped quote no longer
  terminates the literal, and an unknown escape errors. Long-bracket `[[ … ]]` strings still do
  **not** decode (verified). **Deferred:** the multi-character forms `\xHH`, `\ddd`, `\u{XXXX}`,
  and line-continuation `\z` are not handled yet. Tests: `string_escape_sequences`
  (`tests/strings.rs`).

### 2.6 `break` and `repeat … until` unimplemented ✅ FIXED
- **Repro (break):** `for i=1,10 do if i>5 then break end ... end` → `Cannot > 'nil' and 'integer'`
  (the loop variable is corrupted; break is not handled and the stack desyncs).
- **Repro (repeat):** `local i=0 repeat i=i+1 until i>=5 return i` → `Cannot + 'nil' and 'integer'`.
- **Expected:** `break` exits the innermost loop; `repeat`/`until` runs the body once then
  tests (and `until`'s condition can see locals declared in the body).
- **Root cause:** the statement dispatcher had no `Token::Break`/`Token::Repeat` arms.
- **Resolution (2026-06):**
  - Added a `Compiler.loops: Vec<LoopCtx>` stack. Each `LoopCtx` records the `local_count`
    *before* the loop pushed its control/body slots plus a list of pending `break` jump indices.
    `begin_loop`/`end_loop` bracket each loop; `end_loop` patches every break to the loop exit.
  - `break` emits `POPS(local_count - base)` to unwind the loop's live runtime slots (the numeric
    `for`'s 3 hidden control values + loop var + any body locals; a `while`/`repeat` body's
    locals), then a `FORWARD(0)` recorded for patching. Errors (`InvalidTokenPlacement`) outside a
    loop.
  - `repeat … until`: body compiled in an open scope so `until` sees its locals; `GOTO_IF_TRUE`
    (peek) exits on a true condition, otherwise `REWIND`. The cond bool and body locals are popped
    on both the repeat and exit paths.
  - **Bonus:** `while` now scopes its body (`begin_scope`/`end_scope`), fixing a pre-existing leak
    where body locals accumulated across iterations and corrupted slot indices
    (`while i<=3 do local x=i*2 … end` returned 6 instead of 12).
  - Tests (`tests/loops.rs`): `loop_with_break`, `while_with_break`, `repeat_until_loop`,
    `nested_break_only_exits_inner`, `repeat_until_with_break`, `repeat_until_sees_body_local`,
    `while_body_local_is_scoped`.

### 2.7 Generic `for … in` (with `pairs`/`ipairs`) ✅ FIXED
- **Repro:** `for k,v in pairs(t) do … end` → parse error (`Expected token ::=`).
- **Expected:** the generic-for protocol: `for vars in explist do … end` calling
  `iterator(state, control)` until nil.
- **Resolution (2026-06) — built on a new native multi-return ABI:**
  - **Native multi-return:** native functions now return `NativeReturn::{Single,Multi}`
    (`src/function.rs`); the CALL handler spreads `Multi` values and adjusts to the caller's
    wanted count. `register_native_multi_function` registers them. This is the foundation that
    `pcall`/`select`/`table.unpack` will also use.
  - **`next`/`pairs`/`ipairs`** (`src/standard.rs`): `next(t,k)` steps the table via a new
    `Table::next_entry` (hashmap order, O(n) resume — unspecified order, matching Lua); `pairs`
    returns `(next, t, nil)`, `ipairs` returns `(iter, t, 0)` where the iterators are constructed
    inline as native multi-return closures.
  - **Generic-for:** `for_statement` branches on `,`/`in` to `generic_for_statement`; the iterator
    triple is forced to 3 values (same remainder logic as multi-assign) and driven by a new
    `FOR_GENERIC { count, exit }` opcode that calls the (native) iterator, advances the control,
    and pushes the loop vars — mirroring the numeric-for stack discipline (`break`, body locals,
    nesting all work). **Limitation:** the iterator must be a native function (covers
    `pairs`/`ipairs`/`next`); custom Lua-closure iterators aren't driven yet.
  - Tests: `tests/iteration.rs` (9), plus un-ignored `iteration_with_pairs` / `generic_for_ipairs`.
  - **`pairs(userdata)` via `__pairs` ✅ (2026-07):** userdata metamethod dispatch was a
    commented-out stub (`UserDataTypedMap::call_meta_method` always returned `UDNoMethodRef`).
    Un-stubbed by storing metamethod closures as `Rc` (was `Box`) so one can be cloned out of
    `VM.userdata_registry`, releasing the borrow, then invoked with `&mut VM` (the closures need
    `&mut VM` but live inside the VM — a direct call is a self-borrow). `vm_integration::call_meta_method`
    now does clone-then-call; `lua_pairs` routes a userdata through `__pairs` (which returns the
    iterator fn; `pairs` supplies `(iter, ud, nil)`), erroring `MetaMethodMissing` if absent.
    `TestEnt` gained a `__pairs` over its x/y/z fields. Tests: `tests/userdata_pairs.rs`.
  - **Metamethod coverage after the un-stub (2026-07):** on the working `call_meta_method`,
    these userdata metamethods now dispatch — **arithmetic** (`__add`/`__sub`/… via the
    `binary_op!` macro; was already routed, now functional), **`__concat`** (CONCAT opcode,
    either operand), **`__tostring`** (`tostring()` + `print()`), **comparisons** `__eq`/`__lt`/
    `__le` (EQUAL/LESS/LESS_EQUAL/GREATER/GREATER_EQUAL, with `>`/`>=` as swapped `<`/`<=`), plus
    **`__pairs`**. Regular methods (`ud:m()`) and field get/set already worked. Tests:
    `tests/userdata_metamethods.rs`. **Still TODO (documented tail, same Rc pattern):** `__call`
    (calling a userdata), `__len` (`#ud`), `__unm` (unary `-`), `__index`/`__newindex` *metamethod*
    fallback for keys not in the registered fields, and `__ipairs`. `NOT_EQUAL` (`~=`) does not
    consult `__eq` for userdata *or* tables (pre-existing; `==` does) — wire alongside a table fix.

### 2.8 `goto` / labels buggy
- **Repro:** `do goto skip ::skip:: end return 1` → `Expected identifier only inbetween label tokens '::'`.
- **Expected:** forward/back jumps to a `::label::` in scope.
- **Root cause:** partial implementation exists (`goto_statement`, `set_goto_label`,
  `src/compiler.rs:2130/2167/2219`) but label tokenization/resolution is incorrect.
- **Priority:** low — `goto` is rare in game scripts. Track but do not block on it.

### 2.10 Chained/nested table field access `t.a.b` (depth ≥ 2) broken ✅ FIXED
- **Repro:** `local t = {a = {b = 7}}; return t.a.b` → `Cannot perform table operations on a
  non-table value (nil)`. Also `local t = {a={}}; t.a.b = 7` (nested set) and the nested table
  literal `{inner = {value = 42}}` fail the same way.
- **Observed:** `t.a` resolves to `nil` once a second `.field` follows.
- **Expected:** `7`. Single-level access (`t.x`, `t['k']`, `t[1]`) and flat constructors work;
  only chained access / nested literals break.
- **Root cause (actual):** the compiler was *correct* — `t.a.b` already emits one
  `TABLE_GET { depth: 2 }` with keys pushed in source order `[t, "a", "b"]`, and nested
  constructors already build sub-tables. The bug was entirely in the VM: `operate_table`
  (`src/lua.rs`) read the keys with `ip.sub(i)`, i.e. **back-to-front** — step 1 used the
  top-of-stack key (`"b"`) instead of `"a"`, so it dereferenced `t["b"]` (nil) and bailed. The
  "nested literal broken" symptom was the same bug surfacing through the depth-2 *read* used to
  verify it, not a constructor fault.
- **Resolution (2026-06):** index the key as `ip.sub(depth - i + 1)` so navigation runs
  left-to-right. Fixes get, set, and arbitrary depth in one line. Tests: `nested_field_read`,
  `nested_constructor`, `nested_field_write`, `deep_chain_read_write` pass.
- **Follow-up (FIXED):** colon-method *calls* on a multi-level receiver (`a.b:m(41)`,
  `a.b.c:m()`) — see §1.2.1 below.

### 2.11 Under-supplied multiple local assignment doesn't nil extras (function scope) ✅ FIXED
- **Repro:** `function t() local a, b, c = 5; if b == nil then return 999 end; return 0 end; return t()`
  → used to return `0` (i.e. `b` was **not** nil). The identical code at top-level scope always
  returned `999`.
- **Expected:** `local a, b, c = 5` binds `b` and `c` to `nil`.
- **Root cause:** function-local frame slot initialization for under-supplied `local` lists did
  not pad missing values with `nil` (interacted with the vararg/local-offset work, same area as §1.1).
- **Resolution (verified 2026-07):** resolved as part of the multi-return/vararg/upvalue work
  (§1.1 neighbourhood). Now returns `999`, and both `b` and `c` read `nil` in function scope.

### 2.12 Descending numeric `for` (negative step) never runs ✅ FIXED
- **Repro:** `local s = 0; for i = 5, 1, -1 do s = s + i end; return s` → `0`.
- **Expected:** `15` (iterates 5,4,3,2,1).
- **Root cause:** `FOR_NUMERIC` (`src/lua.rs`) compared iterator vs. limit with a fixed
  "greater-than ends the loop" test that assumes a positive step, so a negative step failed the
  very first check and the body was skipped.
- **Resolution (2026-06):** `FOR_NUMERIC` now reads the step (top of the `[iterator, limit, step]`
  window) and branches the loop-continue test on its sign — ascending stops once `iterator >
  limit`, descending once `iterator < limit`. `INCREMENT` already adds the (possibly negative)
  step, so countdowns work. Tests: `numeric_for_descending` (`tests/loops.rs`), incl. multi-step
  `for i=10,2,-2` and an empty descending range. (A zero step still loops forever — pre-existing,
  not addressed here.)

### 2.13 Under-supplied multiple-assignment from a call overflows the stack ✅ FIXED
- **Repro:** `function f() return 42 end; local a, b, c = f(); return a` — `f` returns one value
  into three targets.
- **Was:** **stack overflow → `SIGABRT`** (aborted the process). Distinct from §2.11 (silent
  mis-bind for `local a,b,c = 5`); the *call* form recursed/looped unbounded in the VM's
  `CALL`/`NEED`/multi-return adjustment.
- **Expected:** `a = 42`, `b = nil`, `c = nil`.
- **Root cause:** the multi-return arity-adjustment path (`OpCode::CALL(args, want, _)` +
  `NEED`/`VARARG`, `src/lua.rs` ~line 220 and the `RETURN` handler) did not terminate when the
  callee returned fewer values than requested.
- **Resolution (verified 2026-07):** resolved as part of the multi-return/vararg work (§1.1
  neighbourhood). No crash; `a=42, b=nil, c=nil`, and a 2-value callee spreads correctly
  (`local a,b,c = (function() return 1,2 end)()` → `a=1, b=2, c=nil`).

### 2.9 `u64 → Value → u64` conversion overflow ✅ FIXED
- **Was:** `999999u64` round-tripped to `u64::MAX`.
- **Root cause (both directions, `src/value.rs`):** forward `From<u64>`/`From<usize>` used
  `value.max(i64::MAX)` (forced small values *up*) — should be `.min`; and the reverse
  `from_val!` integer branch clamped with `Self::MAX as i64`, which wraps to `-1` for
  `u64`/`usize`, collapsing every value to `-1` → `u64::MAX`.
- **Fix:** forward `.max` → `.min`; reverse clamp widened to `i128` so `Self::MAX` can't wrap.
  `test_macro_conversions` un-ignored + extended with `usize` and above-`i64::MAX` saturation
  cases. Low severity (Rust-embedding API only, not a hot path).

### 2.14 `call_fn` / `call_with_params` drops runtime arguments ✅ FIXED
- **Was:** args passed to a loaded chunk were lost — `call_fn` pushed them at `base[0..]`, but
  `run → execute` re-seated a fresh `Ephemeral` at the base and `push(Value::Function)`
  overwrote `base[0]`, with `execute` also hardcoding `call_arity = 0`.
- **Fix (`src/lua.rs::call_fn`):** a loaded chunk is a vararg function, so args now arrive as
  `...`. `call_fn` builds the frame itself (instead of via `execute`): args go in the vararg
  region *below* the frame base, the function value sits at the frame base, and
  `frame.call_arity = n` — which is what `VARARG`/`get_varargs` read. Then it runs `process`
  directly and resets `stack_count` after (no drift across repeated calls). 0-arg callbacks
  (loop/main/drop) are unchanged. Tests: `tests/call_fn_args.rs` (varargs delivery, missing→nil,
  ignored args, repeated game-loop calls).
- **Still open (separate, pre-existing):** `return ...` at chunk top level returns the function
  value instead of forwarding the varargs — a return-position multi-value gap, not the arg-loss
  bug. Reading args via `local w, h = ...` (the normal callback pattern) works.

---

## 3. Missing standard library

**Tier 1–3 landed (2026-06):** the single-return base/math/string functions are implemented in
`src/standard.rs` and registered in `load_standard_library`.

- **Base ✅:** `type`, `tostring` (scalars via `coerce_string`; reference types now get a
  Lua-style `table: 0xADDR` / `function: 0xADDR` identity so distinct tables/functions no longer
  compare equal — 2026-07), `tonumber` (incl. base arg + `0x`), `assert`, `error`,
  `next`, `pairs`, `ipairs` (multi-return — see §2.7). Still TODO: `pcall`/`xpcall`, `select`,
  `rawget`/`rawset`/`rawequal`/`rawlen`, `unpack` — all now unblocked by the native multi-return
  ABI.
- **`math` ✅:** `floor`, `ceil`, `abs`, `sqrt`, `sin`/`cos`/`tan`, `min`, `max`, `random`,
  `randomseed` (thread-local xorshift, no `rand` dep), `huge`, `pi`, `maxinteger`, `mininteger`.
  `modf` ✅ (returns integral+fractional as a typed tuple — the first consumer of the
  mlua-style multi-return reshape, see below). Still TODO: `fmod`, `tointeger`, `type`.

  **Native multi-return reshape (2026-07, `value.rs`):** `ToLuaMulti` now follows mlua's
  model — blanket `impl<T: ToLua> ToLuaMulti for T` (single→one value), explicit tuple
  impls (spread), collections stay one table via their `ToLua` impls. Tuples deliberately
  do NOT implement `ToLua` (removed the old tuple→table impl), which is what lets the
  blanket and the tuple spread impls coexist. `NativeFunctionRaw::new` and
  `register_native_function[_to]` now bind `R: ToLuaMulti` and take `F: … -> Result<R, _>`
  (the `?` handles errors; `R` is the success type). `ToLuaMulti::to_native_return` keeps
  the scalar path allocation-free (`Single`, no Vec) — and is the seam a future push-based
  `push_multi` slots into (PLAN §6.1). Tests: `tests/multi_return.rs`.
- **`string` ✅:** `len`, `sub`, `upper`, `lower`, `rep`, `reverse`, `byte` (single-index only —
  see below), `char`, `format` (`%d %i %u %x %X %o %f %e %g %s %c %q %%` with `- + space # 0`
  flags + width + `.precision`). **String metatable wired** — `("hi"):upper()` and `s:method()`
  resolve through the `string` table via `METHOD_GET` (and a new `Token::Colon` Pratt infix so a
  non-identifier receiver like `("x"):m()` works). Still TODO: patterns (`find`/`match`/`gmatch`/
  `gsub`) — a substantial sub-project.
- **`table`:** `insert` ✅ (append `insert(t,v)` + positional `insert(t,pos,v)` with shift-up),
  `remove` ✅ (default removes last `#t`; positional `remove(t,pos)` shifts the rest down;
  returns the removed value, nil on empty — fixed 2026-07, was calling `insert`/`push` and never
  removed anything), `concat` ✅ (with/without separator). Still TODO: `sort`, `unpack`, `pack`.
  All in `tests/tables.rs`.
- **`os`/`io`:** not started.

**Native multi-return ✅ DONE** (§2.7) — `next`/`pairs`/`ipairs` + generic-for shipped on it.
Remaining multi-return consumers still to implement: `pcall`/`xpcall`, `select`, `string.byte`
range form, `table.unpack`/`pack`. The ABI (`NativeReturn::Multi` + `register_native_multi_function`)
is in place, so these are now straightforward.

---

## 3.5 Static typing (Luau-style) — Phase 1 landed, behind `typing` feature (off by default)

Goal: optional, gradual static types that error at **compile time**. The compiler is single-pass
and AST-free, so checking will be done by a "type stack" mirroring the operand stack (woven into
emit), not a separate pass. Types are compile-time only and never reach the VM.

- **Phase 1 (done, 2026-06):** `src/types.rs` (gated `#[cfg(feature = "typing")]`) defines the
  `Type` lattice (`Any`/`Nil`/`Boolean`/`Number`/`String`/`Table`/`Function`/`Optional`/`Named`),
  `Type::from_name`, and the `assignable()` compatibility chokepoint (gradual: `Any` and
  unresolved `Named` are compatible with everything). `Local` gained a `ty` field. Annotations
  are **parsed and stored** — `local x: T`, `local a: T, b: U`, and `function f(a: T)` — via
  `parse_type_annotation`, wired into `named_variable` and `build_param`. **No checking yet:**
  typed code runs with identical dynamic semantics. With the feature off there is zero impact
  (the colon stays a method-call operator). Tests: `tests/typing.rs`.
- **Deferred:** return-type annotations (`function f(): T`) — the param `)` is currently absorbed
  as a void-prefix no-op in the body block, so return types wait for Phase 3 (signatures). Also
  `T?`/unions/table-shapes/generics, and the lexer `?` token.
- **Next phases** (full detail in `TYPING_PLAN.md`):
  - **(1.5) Declaration pre-scan.** The single-pass, AST-free compiler can't resolve a type used
    before its definition. Add a cheap first scan that collects only `type` aliases and top-level
    function signatures into a type environment — ignoring expression bodies — so forward
    references and mutually-recursive aliases work (as in Luau) *without* a full AST. The existing
    single-pass emit then checks against that pre-built environment. This is the chosen approach
    over a full AST rewrite (we stay single-pass for codegen).
  - **(2) Cheap high-confidence checks** via a "type stack" mirroring the operand stack:
    annotated-assignment mismatch, arithmetic on known-non-number, call/index of
    known-non-callable/-table.
  - **(3) Function signatures + arg/return checking** (uses the pre-scan environment).
  - **(4) Optionals/unions/table-shapes/generics + flow narrowing + `--!strict` gating.**
- Luau reference: builds a full AST and type-checks in a pass *separate* from codegen; hoists type
  aliases per-scope (forward + mutually-recursive); infers local types; gradual (`any`) with
  strict/nonstrict modes; flow-sensitive refinement. We approximate this within a single-pass VM
  via the pre-scan + type-stack rather than a full AST.
- The dead `ColonIdentifier`/`Typer`/`colon_blow` lexer machinery is intentionally left in place
  for now (it doesn't conflict); retire it in favor of plain `Token::Colon` when typing matures.

---

## 3.6 Arrow functions (Luau/C#-style) — landed, behind `arrow` feature (on by default)

`params -> body` first-class function expressions.
- **Syntax:** single bare param `x -> …`; multiple params parenthesized `(a, b) -> …`; also
  `(x) -> …` and zero-param `() -> …`. Body is a single expression OR a `do … end` block
  (multi-statement). **Always implicitly returns** its last expression, regardless of the
  `implicit-return` flag.
- **Detection (single-token lookahead, not var_stack unwind):** `variable()` spots `ident ->`
  for the bare single param; `grouping()` delegates a `(`-then-identifier to `grouping_or_arrow`,
  which disambiguates `(a, b) -> …` / `(a) -> …` (arrow) from `(a)` (grouped var) and `(a + b)` /
  `(x -> …)` (ordinary grouped expression). The var_stack-unwind approach the task suggested is
  infeasible here because getters are emitted eagerly before `->` is seen; lookahead is robust and
  the end behavior is identical.
- **`build_arrow_function`** mirrors `build_function`'s scope/closure machinery but takes the
  param names directly and parses the flexible body. Two subtleties found via testing: (1) the
  body must emit a leading `POP` — a normal function body opens with a POP of the parser-absorbed
  `)` token, which the VM's call convention relies on for frame alignment; an arrow has no `)`, so
  it emits the POP explicitly. (2) the single-expression body uses `expression_single` (not
  `expression`) so a trailing comma ends the arrow rather than greedily eating the next argument
  (`f(x -> x*2, 5)` → two args).
- **Feature/flag:** cargo feature `arrow` (in default `silt` set) gates the `->` lexer token, the
  `LanguageFlags::arrow_functions` field, and all parser code; the runtime flag defaults from the
  feature. Tests: `tests/arrow.rs` (single/multi param, do-block, as-argument, closures, currying,
  IIFE, plus grouping-still-works regressions).
- **Typed params (with `typing`):** `(a: number, b: number) -> a + b` parses — `grouping_or_arrow`
  treats a `:` after a param ident (or a `,`) as an arrow parameter list and consumes the
  annotations. The annotation is currently parsed-and-discarded (not yet recorded on the param
  local — a follow-up for the checking phase). The `(a: …` ambiguity with a parenthesized method
  call is **resolved**: after `(ident: name`, a following `(` means a method call, so it falls out
  of param-list mode and recovers as an ordinary grouped expression (`(t:get())`, `(t:get() + 1)`
  both work) via `finish_grouped_method_call` + the extracted `infix_loop` helper. Having committed
  to the method-call interpretation, a trailing `->` (`(t:m()) -> …`) is a clean hard error.
- **Deferred:** multi-value single-expression returns (use a `do` block); recording arrow param
  types on their locals.

## 3.7 Vectors (glam-backed 2D/3D/4D) — landed, behind `vector` feature (off by default)
First-class immutable f32 vector value type for game/math scripting, replacing the old broken
`vectors` stub. `vector = ["dep:glam"]`.
- **Types** (`src/vec.rs`): newtype wrappers `Vec2/Vec3/Vec4` around `glam::Vec{2,3,4}` — needed
  for the orphan rule (`Collect` via manual `unsafe impl`, model `UDVec`) and a Lua `Display`.
  `Copy`, `Deref` to glam, operator impls. Re-exported: `silt_lua::{Vec2,Vec3,Vec4}` and
  `pub extern crate glam` (so embedders name the types without their own glam dep).
- **`Value`/`ExVal`** gained `Vec2/Vec3/Vec4` variants (`src/value.rs`), extended across every
  match (conversions, both `Display`s, `to_error`, `type_name`→`"vec2/3/4"`, clone, `PartialEq`,
  `is_equal`). `ToLua`/`FromLua` for the wrappers AND raw `glam::Vec*` — the entity-interop glue.
- **VM** (`src/lua.rs`): `binary_op!` gets vec·vec (`+ - *`) + scalar broadcast (`+ - *`, both
  operand orders — the scalar applies to every component, GLSL/glam-style); DIVIDE gets vec·vec,
  vec/scalar, and scalar/vec; NEGATE gets unary `-`. `TABLE_GET` returns `.x/.y/.z/.w`;
  `METHOD_GET` dispatches through a global `vec` table (mirrors the `string` library). Registered
  in `load_standard_library`.
- **API:** `vec2/vec3/vec4(...)` constructors; `.x/.y/.z/.w`; `+ - * /` (component-wise and with
  scalars in either order), unary `-`, `==`; `:length() :length_squared() :dot() :normalize()
  :distance() :cross()` (`src/vector_lib.rs`). `type(v)` → `"vec2"/"vec3"/"vec4"`.
- **Entity interop, no metamethod:** a userdata field getter returns a vector, a setter accepts
  one (`v: glam::Vec3`), so `entity.pos = entity.pos + vec3(0,10,0)` works. Tests: `tests/vectors.rs`.
- **Out of scope (future):** swizzling (`v.xy`), `DVec` f64 family, `%`/`^`/`//`, matrices/quats,
  flexible constructors (splat / `vec3(v2, z)`).

**Parser: field/index access on grouped/call results (2026-07).** `.`/`[` are now Pratt infix
operators (`dot_infix`/`index_infix`, `Call` precedence, `src/compiler.rs`), so `(a + b).x`,
`(t)["k"]`, `f().field`, and `(cond and t or u).x` parse — previously "Invalid token placement"
(field access lived only inside `named_variable`'s eager loop for bare variables, which still
handles `t.a.b`). Grouped/call results are rvalues, so `(t).x = 5` correctly errors. Tests:
`tests/tables.rs` (grouped/conditional/call-result/chained access).

## 4. Known deviations (documented, lower priority)

- **`#` on tables returns hashmap length, not a border** (README limitation). Lua's `#`
  returns *a* border for sequences. Revisit when the table type is split into array+hash (§6).
- **No real GC** — reference counting only; self-referential structures leak until VM drop
  (README). `gc-arena` is a dependency; wiring a tracing collector is a separate epic.
- **Integer/float**: correct already; just noting it so future test authors don't "fix" it.

---

## 5. Tail-call optimization (TCO)

Lua **guarantees** proper tail calls: `return f(args)` must reuse the current call frame so
that tail-recursive loops run in O(1) stack. Today every call pushes a new `CallFrame`
(`src/lua.rs`, the `CALL`/`RETURN` path around lines 220 and 1226), so deep tail recursion
will grow the frame stack and eventually overflow.

**Plan:**
1. In the compiler, detect the pattern `return <call>(...)` and emit a `TAIL_CALL` opcode
   instead of `CALL` + `RETURN`.
2. In the VM, `TAIL_CALL` reuses the current frame: overwrite the current frame's slot window
   with the callee + args, reset `ip`, and **do not** push a new `CallFrame`.
3. Interacts with multiple-returns and varargs — land those first (they are WIP on this
   branch). Add tests: a tail-recursive `loop(n)` that would overflow without TCO (e.g.
   `n = 1e7`) must complete with flat frame usage.

TCO is cheap to add once the frame layout is stable and is a meaningful correctness item
(some idiomatic Lua relies on it), so schedule it right after §1–§2.

---

## 6. Architecture: stack VM vs. register VM (and what's best for a game engine)

> The README/SPEC currently say "stack-based, will move to register eventually" and also
> "maintain the stack-based approach; do not convert." This section resolves that tension with
> a recommendation.

### Background
PUC-Lua 5.x is a **register-based** bytecode VM: each function has a fixed register window
(a slice of the stack); instructions address operands by register index (`ADD A B C` →
`R[A] = R[B] + R[C]`). Silt is currently a **stack-based** VM: operands are pushed/popped
(`CONSTANT`, `ADD` pops two and pushes one).

### Trade-offs

| Dimension | Stack VM (current) | Register VM (PUC-Lua) |
|---|---|---|
| Instructions per expression | More (explicit push/pop) | Fewer (operands addressed in place) |
| Dispatch count (the dominant cost) | Higher | ~30–40% lower on typical Lua (well-documented) |
| Compiler complexity | Lower | Higher (register allocation, lifetime tracking) |
| Local variable access | Stack-slot indexed (already register-like here) | Register indexed |
| Constant folding / peephole | Harder | Easier (operands explicit) |
| Closures/upvalues | Same conceptual model | Same |
| Memory traffic | More stack churn | Less |

A key observation: **silt is already half-way to a register model.** Locals are accessed by
slot index (`GET_LOCAL{index}`, `SET_LOCAL{index}`, `INCREMENT{index}`), and `CALL` carries an
arity. The "stack" here is really a register file that arithmetic temporaries also live on.

### Recommendation for a game-engine workload

Game scripting is **dispatch-bound and called every frame** (per-entity `update`, lots of
small arithmetic, table field access, short function calls). That is exactly the workload a
register VM optimizes — fewer dispatches per expression is a direct frame-time win, and the
classic Lua benchmarks (fib, nbody, tight loops) show the register design ahead.

**However**, a full rewrite to a register VM is high-risk while §1–§2 correctness bugs exist
and multiple-returns/varargs are still WIP. The pragmatic path:

1. **Stabilize the stack VM first** (fix §1–§2, land multi-return/vararg, add TCO, build out
   the suite + benchmarks). A correct stack VM beats a fast broken one.
2. **Harvest the cheap register-style wins inside the stack VM** (see §6.1) — these recover a
   large fraction of the register advantage with far less risk.
3. **Then** evaluate a register backend *behind the existing opcode/compiler boundary*, gated
   by the benchmark suite (§7). Because locals are already slot-indexed, the migration is more
   "make temporaries register-addressed" than a ground-up rewrite. Keep the stack VM until the
   register VM is provably faster *and* passes the full suite.

**Bottom line:** register-based is the better long-term target for a game engine, but it is a
*later* milestone gated on (a) a green conformance suite and (b) the benchmark harness showing
a real win. Do not start the rewrite until both exist.

### 6.1 Robustness/efficiency wins for the current stack VM (do these regardless)
- **Native-call arg marshalling — args-side DONE (2026-07).** `popn` used to heap-allocate a
  fresh `Vec` for `fn + args` on **every** native call. Now the CALL handler moves them into a
  **reused `arg_scratch` buffer** on the VM (`mem::take`-d out during the call to avoid aliasing
  `&mut VM`, moved not cloned so no String/Gc clone, restored after). Measured on
  `benches/interpreter.rs`: `native_call_1e6` **68 ms → 56 ms (~18%)**, `native_multi_1e6`
  **84.5 ms → 78.5 ms (~7%)**. The single-return path is now allocation-free end to end (args
  reused + `NativeReturn::Single` from Increment 1).
- **Results-side push (optional remaining).** Multi-return native calls still allocate the
  `NativeReturn::Multi(Vec)`. Eliminating it means the function **pushes results straight onto
  the operand stack, returning a count** (Lua C API style) — the `to_native_return` seam is
  already in place for this. It needs threading the stack handle (`Ephemeral`/`ip`) into the
  native ABI and reconciling `ip` with `stack_count` after the push, so it's riskier for a
  smaller, rarer gain. **Benchmark-gated:** adopt when multi-return native calls show up hot in a
  real workload. Doing it behind an abstracted `push`/`take_arg` accessor also yields the
  `safe`-feature (bounds-checked, no-`unsafe`) VM backend as a drop-in.
- **Fixed-size, overflow-checked stack** instead of an unbounded `Vec` push/pop (SPEC asks for
  this) — predictable latency, no realloc spikes mid-frame.
- **Stop pushing `Nil` on pop** (`src/lua.rs` has many "pushing nil is stupid" TODOs around
  830-848) — eliminates redundant writes; track a real top pointer.
- **Superinstructions / peephole** for hot pairs (`GET_LOCAL`+`ADD`, the existing `INCREMENT`
  is exactly this idea — generalize it).
- **Intern strings end-to-end** (the `string-interner` dep is present) so table-key and global
  lookups compare/hashed by id, not by content (`src/lua.rs:463` TODO).
- **Split table storage into array-part + hash-part** (README/`src/value.rs:71` TODO) — fixes
  `#` semantics (§4) and speeds sequence access, the common game case.
- **Order the opcode `match` by frequency** and consider `#[inline]` on hot handlers
  (`src/lua.rs:1051` TODO).
- **Reduce `OpCode` size** (currently 4 bytes; code.rs notes the size bottleneck) and/or move
  to a packed wordcode for better i-cache behavior.

---

## 7. Test suite organization & CI

`tests/` is organized by feature domain. Files:

- `arithmetic.rs` — +,-,*,/, precedence, unary, int/float typing (live);
  `%`,`^`,`//`,bitwise (ignored → §1.3/§2.1); paren-trailing-op (ignored → §2.2).
- `comparisons.rs` — numeric (live); string relational (ignored → §2.4).
- `conditionals.rs` — if/else/truthiness/logical (live); `elseif` (ignored → §2.3).
- `loops.rs` — while + numeric for (live, corrected to Integer); `break`, `repeat`,
  generic-for (ignored → §2.6/§2.7).
- `functions.rs` — calls, recursion, multiple-return (live where working).
- `closures.rs` — upvalue capture, counters, for-capture (ignored → §1.1).
- `strings.rs` — literals, concat, length (live); escapes + comparison (ignored → §2.4/§2.5).
- `tables.rs` — constructors, indexing, dot/bracket, nesting (live, corrected); `pairs`
  iteration (ignored → §2.7/§3).
- `varargs.rs` — `...` spread, capture-to-table (live where working).
- `stdlib.rs` — `type`/`tostring`/`tonumber`/`pcall`/`math.*`/`string.*` (ignored → §3).
- `metatables.rs` — `__index`/`__add`/method dispatch (ignored where unimplemented).
- `errors.rs` — operations that must raise specific `SiltError`s.

**Goal state:** `cargo test` is green because every *live* test passes and every *broken*
feature is `#[ignore]`d with a `PLAN.md §X` reason. As each bug in §1–§3 is fixed, delete the
corresponding `#[ignore]` — that is the definition of done for that item.

**Proposed CI (`.github/workflows/ci.yml`, not yet created):**
```yaml
# build + test on stable; benchmarks are a SEPARATE, manual job (see §8) and never gate merges
jobs:
  test:
    steps:
      - run: cargo test --all-features      # never runs benches (see §8)
```

> ✅ `cargo test -- --ignored` is now **safe** (§1.3 fixed: `synchronize()` always makes forward
> progress, so a stray/unlexable char errors instead of hanging). It can be wired as an
> informational, non-blocking job. Note one remaining `#[ignore]`d stub — `chunk_validity` in
> `src/lib.rs` — intentionally `panic!`s ("direct hand-built Chunk execution not wired up yet"),
> so `--ignored` reports one expected failure unrelated to language conformance.

---

## 8. Benchmark batch (excluded from normal CI)

Benchmarks live in `benches/` and are **never run by `cargo test`** — `cargo test` does not
build or execute `[[bench]]` targets, so the normal CI job above is unaffected. They run only
via `cargo bench` (a separate, manual/scheduled job).

- `benches/interpreter.rs` — Criterion benchmarks for the hot paths the prompt calls out:
  numeric `for` loops, nested-closure recursion (`fib`), tight `while` loops, string concat,
  table read/write. Criterion is a **dev-dependency**, so it never affects library or release
  builds.
- `benchmarks/*.lua` — the equivalent scripts, shared with PUC-Lua.
- `benchmarks/compare.sh` — runs each script under `lua` (PUC-Lua) and under the `silt` binary,
  printing a side-by-side wall-clock table. This is the "compare against PUC-Lua" tool.

**Regression-gating later:** once §1–§2 are fixed and timings are stable, a *separate*
scheduled CI job can run `cargo bench` and compare against a committed baseline. Keep it out of
the PR-blocking path until CI timing is proven consistent (the prompt's own caveat).

---

## 9. Suggested order of execution

1. **§1.3 parser forward-progress** (kills the hang class) + **§2.2 grouping `)`** (tiny, huge payoff).
2. **§1.1/§1.2 upvalue resolver** (restores closures — headline feature).
3. **§2.1 operators** (`%`,`^`,`//`,bitwise opcodes) + **§2.3 elseif**.
4. **§2.4/§2.5 strings** (comparison + escapes) + **§2.6 break/repeat**.
5. **§3 base stdlib** (`type`,`tostring`,`tonumber`,`pcall`,`assert`,`select`,`next`) →
   **§2.7 generic-for** + `pairs`/`ipairs`.
6. **§3 math/string/table libs** (parallelizable).
7. **§5 TCO** (after multi-return/vararg stabilize).
8. **§6 benchmark-gated register-VM evaluation** (only after the suite is green).

Each fix: remove the matching `#[ignore]`, watch it pass, and update the matching `PLAN.md`
section to ✅.
