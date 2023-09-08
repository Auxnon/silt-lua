# Silt-Lua, a lua subset interpreter in 100% rust

This project exists to answer a problem with the current rust landscape in lacking a complete lua interpreter solution written in 100% rust. The existing implementations as of 2023 are missing crucial features and optmiziations to satisfy the requirements of my closed source project [Petrichor64](https://makeavoy.itch.io/petrichor64), so here we are.

Core focus is a basic interpreter with minor overhead and reletive speed comparison to standard lua. The goal is for a perfect wasm32-unknown-unknown target build. No emscripten necessary! Userdata will mirror traits used by the mlua and rlua crates to allow easy drop in.

Secondary goals are CLI, LSP, and full standard library compliance. Ideally loading third party libraries as well. There's also a concern for safety, as a number of unsafe code is presence in the VM. In the future a safe and unsafe version of the VM will be hidden under as feature flag, on the assumption the unsafe version will operate marginally faster. Exact benchmarks will have to be determined.

There's also desire to add some custom non-lua syntax pulling syntactic sugar from other languages, such as allowing for `!=`, typing, and maybe arrow functions. This superset of lua will fall under a feature flag and by default be disabled as most users likely just want regular lua. This supset satisfies personal requirements but I'm open for requests if any interest is taken in it. This superset should not conflict with standard lua.

This has been written from the ground up with observations of the lua langauge and documentation, source code has not been referenced so it's very possible the language will have some noticeable differences that will hopefully be ironed out eventually. This includes the byte code, as lua now operates under wordcode. Again, this will eventually change. Feel free to submit an issue for anything particularly glaring. This project is a learning exercise so there is a good deal of naive approaches I'm taking to make this a reality.

## Limitations

- Built as a stack based VM, not registry based, this will change eventually
- Currently lacks a true garbage collector for the time being, some objects use basic reference counting but nothing sophisticated. This means it's **currently possible to create memory leaks in the VM with self-referencing structures liked linked lists**, but simple scripts and config loading should have no issues. Memory "leaks" will still drop when the VM instance is dropped.
- The stack is not well maintained yet. Aside from lack of complete safety guarantee at this time, some values may remain on the stack after being popped. These values are not forgotten and can be overwrote and dropped. Despite this there's a program that peaks in usage will keep that memory it's full run. This is being looked into as the feature flags for safety levels are added.
- Metamethods are still WIP

## WebAssembly

A simple wasm module example can be compiled via wasm-pack. Print calls a global jprintln function if one exists. A live example can be viewed at [MakeAvoy.com](https://MakeAvoy.com/#code)

## Optional Additions (feature flags)

- Upper file flags like --!local force implicit declaration to assign to the current scope instead of at the global level. You can still always declare globals anywhere via the keyword "global", python style
- Anoynmous arrow functions of the -> (C# style) are supported `func_name =param -> param+1` in addition to this arrow functons have implicit returns. The last value on the stack is always returned. Regular functions without a `return` keyword will return nil as before.
- Bang usage ! for not or not equal (~=) can be used if you're hard pressed to not use them like I am. They do not replace not or ~=, only act as builtin aliases
- Incrementers like `+=` `-=` `*=` `/=`
- Numbers can include underscores which are ignored characters used for readability, borrowed right from rust

## Examples

```rust

let source_in = r#"
local d=5
function sum()
local a=1
local b=2
local c=3
return a+b+c+d+8
end

return sum()
"#;

let mut vm = SiltLua::new();
vm.load_standard_library();
match vm.run(source_in) {
    Ok(value) => {
        println!(">> {}", value);
    }
    Err(e) => {
        e.iter().for_each(|e| println!("!!Err: {}", e));
    }
}
```
