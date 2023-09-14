# Silt-Lua, a Lua superset interpreter in 100% rust

This project was originally created to answer a problem with the current rust landscape in lacking a complete lua interpreter solution written in 100% rust. That may not necessarily be true anymore. Even still, the existing implementations at the time were missing crucial features and optimizations to satisfy the requirements of my closed source project [Petrichor64](https://makeavoy.itch.io/petrichor64), so here we are.

Core focus of the library is a basic interpreter with minor overhead and relative speed comparison to standard lua. The goal is for a perfect wasm32-unknown-unknown target build. No emscripten necessary! UserData will mirror traits used by the mlua and rlua crates to allow easy drop in.

Secondary goals are CLI, LSP, and as much standard library compliance as possible. There's also a concern for safety, as a number of unsafe code is present in the VM. In the future a safe and unsafe version of the VM will be hidden under a feature flag, on the assumption the unsafe version will operate marginally faster. Exact benchmarks will have to be determined.

There's also desire to add some custom non-lua syntax pulling syntactic sugar from other languages, such as allowing for `!=`, typing, and maybe even arrow functions. This superset of lua will fall under a feature flag and by default be disabled as who really wants my opinionated concept of a programming language? This superset satisfies personal requirements but I'm open to requests if an interest is shown. Even if this superset is enabled it will not conflict with standard lua.

This library has been written from the ground up with observations of the lua language and documentation. Source code has not been referenced so naturally the VM will always have some noticeable differences that will hopefully be ironed out eventually. This includes the byte code, as lua now operates under wordcode to work with it's register based VM. Feel free to submit an issue for anything particularly glaring. This project is a learning exercise so there is a good deal of naive approaches I'm taking to make this a reality.

## Limitations

- Built as a stack based VM, not register based, this will change eventually.
- Multiple returns is still WIP
- Currently lacks a true garbage collector for the time being, some objects use basic reference counting but nothing sophisticated. This means it's **currently possible to create memory leaks in the VM with self-referencing structures liked linked lists**, but simple scripts and config loading should have no issues. Memory "leaks" will still drop when the VM instance is dropped.
- The stack is not well maintained yet. Aside from lack of a complete safety guarantee at this time, some values MAY remain on the stack after being popped. These values are not forgotten and can be overwrote and dropped. Extensive testing is still needed. Despite this there's a program that peaks in usage will keep that memory it's full run. This is being looked into as the feature flags for safety levels are added.
- Metamethods are still WIP
- Tables use a hashmap and although mixing indexed values with non will generate correctly, using `#` will give the actual hashmap length, not the "consecutive length" that lua does.
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
- <del> Anonymous arrow functions of the -> (C# style) are supported `func_name =param -> param+1` in addition to this arrow functions have implicit returns. The last value on the stack is always returned. Regular functions without a `return` keyword will return nil as before. </del>
- <del> Incrementors like `+=` `-=` `*=` `/=` </del>

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
