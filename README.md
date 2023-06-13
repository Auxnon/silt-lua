## Additions

- Upper file flags like --!local force implicit declaration to assign to the current scope instead of at the global level. You can still always declare globals anywhere via the keyword "global", python style
- anoynmous arrow functions of the -> (C# style) are supported `func_name =param -> param+1` in addition to this functons will return the last statement value
- bang usage ! for not or not equal (~=) can be used if you're hard pressed to not use them like I am. They do not replace not or ~=, only act as builtin aliases
- numbers can include underscores which are ignored characters used for readability, borrowed right from rust
