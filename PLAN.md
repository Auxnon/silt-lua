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

### 1.2 Method/field function definition panics 🔴
- **Repro:** `local t = {x=5}; function t:get() return self.x end` — also `function t.get() ... end`.
- **Observed:** Rust panic (same upvalue-resolution path as 1.1).
- **Expected:** defines `t.get` with implicit `self` (colon form).
- **Root cause:** the `function <expr>:<name>()` / `function <expr>.<name>()` path funnels into
  the same broken upvalue resolution. Likely fixed together with 1.1; verify separately.

### 1.3 Bitwise operators hang the compiler ♾️
- **Repro:** `return 6 & 3` (also `|`). The process spins forever.
- **Observed:** infinite loop; no error, no result.
- **Expected:** Lua 5.3 integer bitwise: `6 & 3 == 2`, `4 | 1 == 5`, `5 ~ 1 == 4`,
  `~0 == -1`, `1 << 4 == 16`, `256 >> 2 == 64`.
- **Root cause:** `&`, `|`, `^` (and the binary `~`/shift forms) are **not lexed** — the lexer
  (`src/lexer.rs`, char match ending at the catch-all `cw =>` at line 648) has no arm for `&`
  or `|`, so they hit `SiltError::UnexpectedCharacter`, eat one char, and the parser's
  precedence loop fails to advance past the error token, spinning. **A hang is worse than an
  error** — at minimum the parser must make forward progress on an unexpected character.
- **Fix sketch:** (a) make the parser error-recovery always advance the token cursor (kills the
  hang for *any* future stray char); (b) lex `&`, `|`, `<<`, `>>`, and binary `~`; (c) add the
  opcodes in §2.1.

---

## 2. Major correctness bugs

### 2.1 Arithmetic operators with no opcode: `%`, `^`, `//` 🔴
- **Repro:** `return 5 % 2` → `5`; `return 17 % 5` → `17`; `return 2 ^ 10` → `2`;
  `return 7 // 2` → error/garbage.
- **Observed:** the operator is silently dropped; the result is the **left operand**.
- **Expected:** `5 % 2 == 1`, `5.5 % 2 == 1.5` (floored modulo), `2 ^ 10 == 1024.0`
  (`^` always float), `7 // 2 == 3` (floor division).
- **Root cause:** there are **no `MODULUS`, `POWER`, `FLOOR_DIVIDE`, or bitwise opcodes** in
  `src/code.rs` (`OpCode` enum). In `binary()` (`src/compiler.rs:2934`) only `ADD/SUB/MULTIPLY/
  DIVIDE/CONCAT/comparisons` are emitted; everything else falls through to `_ => todo!()`
  (line 2964) — but because `%`/`^`/`//` have no infix **rule** in the Pratt table they never
  reach `binary()`; the precedence loop just stops and the trailing tokens are abandoned.
- **Fix sketch:** add `MODULUS`, `POWER`, `FLOOR_DIVIDE`, `BIT_AND`, `BIT_OR`, `BIT_XOR`,
  `BIT_NOT`, `SHIFT_L`, `SHIFT_R` opcodes; wire their precedence rules (`^` is
  right-associative and binds tighter than unary; `%`,`//` share `*`/`/` precedence; bitwise
  sit between comparison and concat per the 5.3 grammar); implement in the VM with Lua's
  integer/float coercion rules (`%` is floored, `^` always float, bitwise require integer
  representability or error).

### 2.2 Parenthesized sub-expression drops trailing operators 🔴
- **Repro:** `return (1+2)*3` → `3`; `return (10+5)*2-3` → `15`; `return (1+2)+3` → `3`.
  (Note: `2*(1+2)` → `6` works, because the paren group is not followed by another operator.)
- **Observed:** any operator *after* a closing `)` is dropped; the value is just the group.
- **Expected:** `(1+2)*3 == 9`, `(10+5)*2-3 == 27`.
- **Root cause:** `grouping()` (`src/compiler.rs:2751`) calls `expression()` but **never
  consumes the closing `)`** — the `expect CloseParen` is commented out (lines 2760-2762). The
  stray `)` then sits at the cursor with no infix rule, so the enclosing precedence loop halts
  and the following operator is never parsed.
- **Fix sketch:** consume `)` at the end of `grouping()` (re-enable the `expect_token!(... CloseParen ...)`),
  and add an `UnterminatedParenthesis` error path. This is a small fix with large blast radius —
  prioritize it.

### 2.3 `elseif` fails to parse 🔴
- **Repro:** `local x=2; if x==1 then return 10 elseif x==2 then return 20 else return 30 end`
- **Observed:** `Expected token: then`.
- **Expected:** returns `20`.
- **Root cause:** `if_statement()` (`src/compiler.rs:1985`) eats the leading keyword with
  `this.eat(it)` at line 1987, then for `elseif` it recurses into `if_statement()` *after*
  already eating the `ElseIf` token (line 2002-2005). The recursive call eats the **first
  token of the condition** as if it were the `if` keyword, so `elseif x==2 then` loses `x` and
  the parser then expects `then` where `==` is.
- **Fix sketch:** split the keyword-eat out of `if_statement`, or pass a `already_ate: bool`,
  so the `elseif` branch parses the condition from the correct position.

### 2.4 String relational comparison unimplemented
- **Repro:** `return 'abc' < 'abd'` → `Cannot < 'string' and 'string'`.
- **Expected:** lexicographic: `'abc' < 'abd' == true`, `'b' > 'a' == true`, `'abc' <= 'abc' == true`.
- **Root cause:** the `LESS`/`LESS_EQUAL`/`GREATER`/`GREATER_EQUAL` opcode handlers in the VM
  (`src/lua.rs`) only accept numeric operands. (`==`/`~=` on strings already work.)
- **Fix sketch:** extend the comparison opcodes to compare two `Value::String` lexicographically;
  keep the type-error for mixed string/number per Lua.

### 2.5 String escape sequences not decoded
- **Repro:** `return 'a\nb'` → the literal three chars `a`, `\`, `n`, `b` (backslash kept).
- **Expected:** `\n`, `\t`, `\\`, `\"`, `\'`, `\r`, `\0`, `\xHH`, `\ddd`, `\u{XXXX}`, `\z` decoded.
- **Root cause:** `Lexer::string()` (`src/lexer.rs`, around the `'"' =>`/`'\'' =>` arms) copies
  the raw slice without an escape-decoding pass. (Long-bracket strings `[[ ... ]]` correctly do
  **not** decode escapes — keep that.)
- **Fix sketch:** decode escapes while scanning quoted strings; error on invalid escapes.

### 2.6 `break` and `repeat … until` unimplemented
- **Repro (break):** `for i=1,10 do if i>5 then break end ... end` → `Cannot > 'nil' and 'integer'`
  (the loop variable is corrupted; break is not handled and the stack desyncs).
- **Repro (repeat):** `local i=0 repeat i=i+1 until i>=5 return i` → `Cannot + 'nil' and 'integer'`.
- **Expected:** `break` exits the innermost loop; `repeat`/`until` runs the body once then
  tests (and `until`'s condition can see locals declared in the body).
- **Root cause:** the statement dispatcher (`src/compiler.rs`, around line 1850) has **no
  `Token::Break`, `Token::Repeat`, or `Token::Until` arms**. `while` (`while_statement` 2014)
  and numeric `for` (`for_statement` 2038) exist and work for the simple cases.
- **Fix sketch:** implement `break` as a forward jump patched to the loop's end, tracked on a
  per-loop "break list"; implement `repeat` as a backward jump with the `until` condition
  compiled in the body's scope.

### 2.7 Generic `for … in` (with `pairs`/`ipairs`) unimplemented
- **Repro:** `for k,v in pairs(t) do … end` → parse error (`Expected token ::=`).
- **Expected:** the generic-for protocol: `for vars in explist do … end` calling
  `iterator(state, control)` until nil.
- **Root cause:** `for_statement` only handles the numeric form; the generic form (multiple
  loop vars + `in`) is not parsed, and `pairs`/`ipairs`/`next` don't exist (§3). CHANGELOG
  already flags "Generic for still WIP".
- **Fix sketch:** parse `for namelist in explist do`; emit the generic-for loop opcodes
  (`TFORCALL`/`TFORLOOP` analog); implement `next`, then `pairs`/`ipairs` on top.

### 2.8 `goto` / labels buggy
- **Repro:** `do goto skip ::skip:: end return 1` → `Expected identifier only inbetween label tokens '::'`.
- **Expected:** forward/back jumps to a `::label::` in scope.
- **Root cause:** partial implementation exists (`goto_statement`, `set_goto_label`,
  `src/compiler.rs:2130/2167/2219`) but label tokenization/resolution is incorrect.
- **Priority:** low — `goto` is rare in game scripts. Track but do not block on it.

### 2.10 Chained/nested table field access `t.a.b` (depth ≥ 2) broken 🔴
- **Repro:** `local t = {a = {b = 7}}; return t.a.b` → `Cannot perform table operations on a
  non-table value (nil)`. Also `local t = {a={}}; t.a.b = 7` (nested set) and the nested table
  literal `{inner = {value = 42}}` fail the same way.
- **Observed:** `t.a` resolves to `nil` once a second `.field` follows.
- **Expected:** `7`. Single-level access (`t.x`, `t['k']`, `t[1]`) and flat constructors work;
  only chained access / nested literals break.
- **Root cause:** the `TABLE_GET`/`TABLE_SET` dot-access path (`src/compiler.rs` `dot`/index at
  ~2709, `TABLE_GET{depth}` in the VM) does not keep the intermediate table on the stack for the
  next `.field`, and the table constructor does not recurse into nested `{...}` values.
- **Fix sketch:** make field access leave the resolved sub-table as the receiver for the next
  link; make the constructor evaluate nested table literals as values.

### 2.11 Under-supplied multiple local assignment doesn't nil extras (function scope) 🔴
- **Repro:** `function t() local a, b, c = 5; if b == nil then return 999 end; return 0 end; return t()`
  → `0` (i.e. `b` is **not** nil). The identical code at top-level scope correctly returns `999`.
- **Expected:** `local a, b, c = 5` binds `b` and `c` to `nil`.
- **Root cause:** function-local frame slot initialization for under-supplied `local` lists does
  not pad missing values with `nil` (interacts with the vararg/local-offset work on this branch,
  same area as §1.1).

### 2.12 Descending numeric `for` (negative step) never runs 🔴
- **Repro:** `local s = 0; for i = 5, 1, -1 do s = s + i end; return s` → `0`.
- **Expected:** `15` (iterates 5,4,3,2,1).
- **Root cause:** `FOR_NUMERIC` (`src/code.rs`, handled in `src/lua.rs`) compares iterator vs.
  limit with a fixed "greater-than ends the loop" test that assumes a positive step, so a
  negative step fails the very first check and the body is skipped. Ascending loops and
  positive steps work.
- **Fix sketch:** branch the loop-continue test on the sign of the step (or normalize so the
  comparison direction follows `step`), per the Lua numeric-for semantics.

### 2.13 Under-supplied multiple-assignment from a call overflows the stack 🔴
- **Repro:** `function f() return 42 end; local a, b, c = f(); return a` — `f` returns one value
  into three targets.
- **Observed:** **stack overflow → `SIGABRT`** (aborts the process). This is distinct from §2.11
  (which silently mis-binds for `local a,b,c = 5`); the *call* form recurses/loops unbounded in
  the VM's `CALL`/`NEED`/multi-return adjustment.
- **Expected:** `a = 42`, `b = nil`, `c = nil`.
- **Root cause:** the multi-return arity-adjustment path (`OpCode::CALL(args, want, _)` +
  `NEED`/`VARARG`, `src/lua.rs` around line 220 and the `RETURN` handler) does not terminate
  when the callee returns fewer values than requested. Belongs to the same multi-return/vararg
  work that is WIP on this branch (§1.1 neighbourhood). High severity — it is a hard crash on
  very common code.

### 2.9 `u64 → Value → u64` conversion overflow
- **Repro:** `tests/value_macro_test.rs::test_macro_conversions` — `999999u64` round-trips to
  `18446744073709551615` (`u64::MAX`).
- **Root cause:** the `From<u64>`/`Into<u64>` path in `src/value.rs` mishandles the unsigned →
  `i64` storage round-trip. Low severity (Rust-embedding API only), but it is a real bug.

---

## 3. Missing standard library

Only `print`, `clock`, `setmetatable`, `getmetatable`, `test_ent`, `table.insert`,
`table.remove` are registered (`src/standard.rs`, `src/lua.rs:2280` `load_standard_library`).
`select` is a `todo!()`. For a usable language/game-config runtime, implement at least:

- **Base:** `type`, `tostring`, `tonumber`, `assert`, `error`, `pcall`, `xpcall`, `select`,
  `next`, `pairs`, `ipairs`, `rawget`, `rawset`, `rawequal`, `rawlen`, `unpack`/`table.unpack`,
  `ipairs`. (`pcall`/`error` are needed for any robust embedding.)
- **`math`:** `floor`, `ceil`, `abs`, `sqrt`, `min`, `max`, `huge`, `pi`, `sin`/`cos`/`tan`,
  `random`, `randomseed`, `fmod`, `modf`, `maxinteger`, `mininteger`, `tointeger`, `type`.
- **`string`:** `len`, `sub`, `upper`, `lower`, `rep`, `reverse`, `byte`, `char`, `format`,
  `find`, `match`, `gmatch`, `gsub` (patterns are a substantial sub-project). **Also wire the
  string metatable** so `("hi"):upper()` resolves through `string.*` — currently
  `("hi"):upper()` returns the receiver unchanged.
- **`table`:** `concat`, `sort`, `unpack`, `pack`, `insert`/`remove` (improve existing).
- **`os` (subset):** `time`, `clock`, `date`. **`io` (subset, optional)** for non-wasm.

These are additive (no architectural risk) and well suited to parallel work once §1–§2 land.

---

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

> ⚠️ Do **not** add `cargo test -- --ignored` to CI yet. The `bitwise` test (§1.3) hangs the
> test binary forever because `&`/`|` send the parser into an infinite loop. Run `--ignored`
> manually only after §1.3's parser-forward-progress fix lands; then it becomes safe to wire as
> an informational, non-blocking job.

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
