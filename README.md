# Silt-Lua, a lua subset interpreter in 100% rust

This project exists to answer a problem with the current rust landscape in lacking a complete lua interpreter solution written in 100% rust. The existing implementations as of 2023 are missing crucial features and optmiziations to satisfy the requirements of my closed source project [Petrichor64](https://makeavoy.itch.io/petrichor64), so here we are.

Core focus is a basic interpreter with minor overhead and reletive speed comparison to standard lua. The goal is for a perfect wasm32-unknown-unknown target build. No emscripten necessary! Userdata will mirror traits used by the mlua and rlua crates to allow easy drop in.

Secondary goals are CLI, LSP, and full standard library compliance. Ideally loading third party libraries as well, if under the silt subset then the ability to import libraries as pure lua will be added, in case of any potential overlooked token conflicts. Again, these goals are TBD and not top priority.

As it remains to be completley compliant with the lua language, additonal features are completly optional under the "silt" flag.

This has been written from the ground up with observations of the lua langauge and documentation, source code has not been referenced so it's very possible the language will have some noticeable differences that will hopefully be ironed out eventually. Feel free to submit an issue for anything particularly glaring. This project is a learning exercise so ther is a good deal of naive approaches I'm taking to make this a reality.

## Limitations

- Built as a stack based VM, not registry based, this may change eventually
- Currently lacks a true garbage collector for the time being, some objects use basic reference counting but nothing sophisticated. This means it's **currently possible to create memory leaks in the VM with self-referencing structures liked linked lists**, but simple scripts and config loading should have no issues
- Metamethods are still WIP

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
