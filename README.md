# Silt-Lua, a lua subset interpreter in 100% rust

This project exists to answer a problem with the current rust landscape in lacking a complete lua interpreter solution written in 100% rust. The existing implementations as of 2023 are missing crucial features and optmiziations to satisfy the requirements of my closed source project [Petrichor64](https://makeavoy.itch.io/petrichor64), so here we are.

Core focus is a basic interpreter with minor overhead and reletive speed comparison to standard lua. The goal is for a perfect wasm32-unknown-unknown target build. No emscripten necessary! Userdata will mirror traits used by the mlua and rlua crates to allow easy drop in.

Secondary goals are CLI, LSP, and full standard library compliance. Ideally loading third party libraries as well, if under the silt subset then the ability to import libraries as pure lua will be added, in case of any potential overlooked token conflicts. Again, these goals are TBD and not top priority.

As it remains to be completley compliant with the lua language, additonal features are completly optional under the "silt" flag.

This has been written from the ground up with observations of the lua langauge and documentation, source code has not been referenced so it's very possible the language will have some noticeable differences that will hopefully be ironed out eventually. Feel free to submit an issue for anything particularly glaring. This project is a learning exercise so ther is a good deal of naive approaches I'm taking to make this a reality.

## Additions

- Upper file flags like --!local force implicit declaration to assign to the current scope instead of at the global level. You can still always declare globals anywhere via the keyword "global", python style
- anoynmous arrow functions of the -> (C# style) are supported `func_name =param -> param+1` in addition to this functons will return the last statement value
- bang usage ! for not or not equal (~=) can be used if you're hard pressed to not use them like I am. They do not replace not or ~=, only act as builtin aliases
- numbers can include underscores which are ignored characters used for readability, borrowed right from rust

## Examples

```lua

```
