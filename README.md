

# Silt-Lua, a Lua superset interpreter in 100% rust

This project was originally created to answer a problem with the current rust landscape in lacking a complete lua interpreter solution written in 100% rust. That may not necessarily be true anymore. In fact, you probably want Kyren's [piccolo](https://github.com/kyren/piccolo), which is a much more robust Lua interpreter with less overhead. This project also uses Kyren's [gc-arena](https://github.com/kyren/gc-arena) for garbage collection. Even still, the existing implementations at the time the project started were missing crucial features and optimizations to satisfy the requirements of my closed source project [Petrichor64](https://makeavoy.itch.io/petrichor64). I considered shelving the project but I have too much I'd like to extend the language with, plus writing an interpreter is just plain fun.

Core focus of the library is a basic interpreter with minor overhead and relative speed comparison to standard lua. The goal is for a perfect wasm32-unknown-unknown target build. No emscripten necessary! UserData will mirror traits used by the mlua and rlua crates to allow easy drop in.

Secondary goals are CLI, LSP, and as much standard library compliance as possible. There's also a concern for safety, as a number of unsafe code is present in the VM. In the future a safe and unsafe version of the VM will be hidden under a feature flag, on the assumption the unsafe version will operate marginally faster. Exact benchmarks will have to be determined.

There's also desire to add some custom non-lua syntax pulling syntactic sugar from other languages, such as allowing for `!=`, typing, and maybe even arrow functions. This superset of lua will fall under a feature flag and by default be disabled as who really wants my opinionated concept of a programming language? This superset satisfies personal requirements but I'm open to requests if an interest is shown. Even if this superset is enabled it will not conflict with standard lua.

This library has been written from the ground up with observations of the lua language and documentation. Source code has not been referenced so naturally the VM will always have some noticeable differences that will hopefully be ironed out eventually. This includes the byte code, as lua now operates under wordcode to work with it's register based VM. Feel free to submit an issue for anything particularly glaring. This project is a learning exercise so there is a good deal of naive approaches I'm taking to make this a reality.

## Running

The `silt` binary takes either a file path or an inline program:

```sh
silt path/to/script.lua          # run a file
silt --run "return 1 + 2"        # run a string directly (prints 3)
silt -r 'print("hi")'            # short flag
silt --help                      # usage (alias: -h)
```

With `--run`/`-r`, everything after the flag is treated as the Lua source (quote it so your shell passes it as one argument). The final returned value is printed after a separator line.

## Limitations

> **Conformance status & roadmap:** see [`PLAN.md`](./PLAN.md) for the full audited list of
> working features, known bugs (with reproductions and root-cause pointers), the missing
> standard library, tail-call-optimization notes, and the stack-vs-register VM analysis. The
> test suite under `tests/` encodes the target behavior — broken features have `#[ignore]`d
> tests tagged with the relevant `PLAN.md` section.

- Built as a stack based VM, not register based, this will change eventually.
- Multiple returns is still WIP
- Currently lacks a true garbage collector for the time being, some objects use basic reference counting but nothing sophisticated. This means it's **currently possible to create memory leaks in the VM with self-referencing structures liked linked lists**, but simple scripts and config loading should have no issues. Memory "leaks" will still drop when the VM instance is dropped.
- The stack is not well maintained yet. Aside from lack of a complete safety guarantee at this time, some values MAY remain on the stack after being popped. These values are not forgotten and can be overwrote and dropped. Extensive testing is still needed. Despite this there's a program that peaks in usage will keep that memory it's full run. This is being looked into as the feature flags for safety levels are added.
- Metamethods are still WIP
- Tables keep the sequence `1..n` in a dense array part and other keys in a hash part (as PUC-Lua does); `#t` is the array border. A table with holes may report any border, as in Lua.
- Standard library is only `print` and `clock` function for testing. Feel free to utilize `register_native_function` on the VM instance to fill any gaps for now

## WebAssembly

A simple wasm module example can be compiled via wasm-pack. Print calls a global jprintln function if one exists. A live example can be viewed at [MakeAvoy.com](https://MakeAvoy.com/#code)

## Optional Additions (feature flags)

Keep in mind these may be polarizing and an LSP will flag them as an error

- `"bang"` Bang usage ! for not or not equal (~=) can be used if you're hard pressed to not use them like I am. They do not replace not or ~=, only act as builtin aliases
- `"under-number"` Numbers can include underscores which are ignored characters used for readability, borrowed right from rust
- `"short-declare"` Stolen right from Go you can now declare a local variable with `:=` such as `a := 2`
- <del>`"implicit-return"` Blocks and statements will implicitly return the last value on the stack unless ending in a `;`</del>
- <del> Top of file flags like --!local force implicit declaration to assign to the current scope instead of at the global level. You can still always declare globals anywhere via the keyword "global", python style </del>
- `"arrow"` Arrow functions: `x -> x + 1` (single param, bare), `(a, b) -> a + b` (multiple params, parenthesized). The body is a single expression, or a `do … end` block for multiple statements; either way the body is **always implicitly returned** (regardless of the `implicit-return` flag). They are first-class expressions — assign them, pass them as arguments, curry them (`x -> y -> x + y`), or call inline (`((x) -> x*x)(9)`). A comma ends an arrow body, so `f(x -> x*2, 5)` passes the arrow and `5` as two arguments.
- `"compound-assignment"` Luau-style compound assignment operators: `+=` `-=` `*=` `/=` `//=` `%=` `^=` `..=`. `x op= e` is shorthand for `x = x op e` and evaluates the target once. Works on locals, upvalues, globals, and table fields/indexes (`t.x += 1`, `t[k] *= 2`, `self.count += 1`).
- `"typing"` (off by default) Luau-style gradual static typing, checked at **compile time** with zero runtime cost: `local x: number`, typed params and return types (`function f(a: number): string`), `T?`, `A | B`, table shapes `{x: number}` / `{T}` / `{[K]: V}`, function types `(T) -> R`, `type Name = T` aliases (usable before their definition), and narrowing in `if x then` / `if x ~= nil` / `if type(x) == "…"`. Nonstrict by default (only annotated boundaries are checked); `--!strict` adds operator/index/call checks and infers unannotated locals. Type errors are compile errors. A host declares its natives' types with `Compiler::declare(name, "(number) -> nil")`, and the `snippet` type (`snippet`, `snippet(number, number) -> number`) marks a function that must be able to leave the VM: a function literal passed where a snippet is expected is checked to capture and reference values only (see `"snippets"`). See `TYPING_PLAN.md`.
- `"snippets"` (off by default) Let a Lua function **escape the VM**: `Snippet::extract(vm, &fn_value)` (from a host native) or `Lua::extract_snippet(src)` captures a function's bytecode plus value-copies of the locals it closed over and the value globals it reads, as plain `Send + Sync` data. Hand it to another thread and `Lua::build_snippet(&s)` there (~5 µs) for an isolated VM you can call repeatedly — `call(&[ExVal])`, or the allocation-free `call_f64(&[f64])` / `render(&mut [f32], …)` for per-sample work like a synth voice. Reference captures (functions, userdata) are rejected at extraction; reference *globals* (`math`, user functions) resolve from the snippet's own VM, and every error is tagged `[snippet]` with a hint naming any such global that came back nil. With `"typing"` the same rules are checked at **compile time**: declare the consuming native as taking a `snippet(...)` and a literal that captures a function or references a script global is a type error naming it, in the editor too. See `SPEC-snippets.md`.
- `"hot-swap"` (off by default) Live code hot-swapping for dev/live-reload tooling. `Lua::hotswap(old, new)` diffs the two sources: a root-level change is a full VM reset; a function-body-only change is applied *live* with no re-run and all state preserved. Changed **top-level global** functions are rebound in place, and changed **nested / instance methods** are redirected via a shared prototype cell so every existing instance picks up the new body on its next call while keeping its own captured state (the unchanged enclosing function is left alone). Not needed for normal embedding, so it's gated off.
- `"lsp"` (off by default) Pure-Rust language-analysis API for editor/tooling integration, returning **native Rust types** (no JSON): `diagnostics` (compile errors with full spans, without running the code), `format` (Lua-style re-indentation), and the editor features `hover`, `definition`, `references`, `rename`, `completion`, `document_symbols`, `signature_help`, `semantic_tokens`, all under `silt_lua::lsp`. Call these directly in-process.
- `"lsp-server"` (off by default, implies `lsp` and `typing`) A JSON-RPC-over-stdio language server for editors like Neovim, built on the `lsp` core (adds `serde_json` only for the wire format). See **Editor integration** below.

## Editor integration (LSP)

silt can act as a language server so editors get live diagnostics and formatting for silt/Lua source — including silt's custom syntax (arrow functions, compound assignment, `!`, etc.) that a stock Lua LSP wouldn't understand.

Build with the server feature and launch over stdio:

```sh
cargo build --release --features lsp-server   # add to defaults: --features "silt lsp-server"
silt lsp                                       # speaks LSP/JSON-RPC on stdio
```

`silt lsp` writes **only** framed JSON-RPC to stdout (logs go to stderr). It provides **diagnostics** (compile and type errors with full spans, on open/change), **formatting** (honoring `tabSize`/`insertSpaces`), **hover** (signature, declared types and the `--` doc comment above a definition), **go to definition**, **find references**, **rename**, **completion** (symbols in scope, the standard library enumerated from the real globals table, `recv.` members seen in the file or declared by the receiver's table-shape type, type names after `:`), **document symbols** (outline), **signature help** (active parameter tracking; `obj:method(` drops `self`) and **semantic tokens**. Scoping follows Lua: shadowing resolves to the innermost definition, loop variables end with their loop.

### Neovim

No plugin required (Neovim 0.8+). Point the built-in client at the `silt` binary:

```lua
-- ~/.config/nvim/after/ftplugin/lua.lua  (or inside a FileType autocmd)
vim.lsp.start({
  name = "silt",
  cmd = { "silt", "lsp" },          -- or an absolute path to the built binary
  root_dir = vim.fs.dirname(vim.fs.find({ ".git" }, { upward = true })[1])
             or vim.fn.getcwd(),
  -- optional: the host's natives, so scripts are type-checked against them
  -- ("declareInSnippet" lists natives the host also provides inside snippet VMs)
  init_options = {
    declare = { instr = "(number, snippet(number, number) -> number) -> nil" },
    declareInSnippet = { emit = "(number) -> nil" },
    -- files or directories whose globals every document can see: sibling scripts
    -- and `---@meta` stubs (`---@param`/`---@return`/`---@type` fold into hovers)
    library = { vim.fn.getcwd() },
  },
})
```

With `library`, hover, completion, signature help and go-to-definition reach definitions in those files (a document's own definitions win). Directories are scanned recursively for `.lua`/`.silt` (skipping hidden dirs, `target`, `node_modules`; at most 500 files), and open buffers update the library live. Built with `vector`, the `vec2/3/4` constructors and `:dot`/`:cross`/… methods are documented too.

Diagnostics then appear automatically via `vim.diagnostic`; format the buffer with `vim.lsp.buf.format()`.

> Note: diagnostics only flag what silt's single-pass compiler detects (unterminated blocks, missing tokens, …); it is permissive about some malformed input. Columns are Unicode-char based — exact for ASCII/BMP source.

## Examples

```rust

let source_in = r#"
do
    local d=5
    function sum()
        local a=1
        local b=2
        local c=3
        return a+b+c+d+8
    end

    return sum()
end
"#;

let mut vm = Lua::new();
vm.load_standard_library();
match vm.run(source_in) {
    Ok(value) => {
        println!(">> {}", value);
        assert_eq!(value, Value:Integer(19));
    }
    Err(e) => {
        e.iter().for_each(|e| println!("!!Err: {}", e));
    }
}
```
