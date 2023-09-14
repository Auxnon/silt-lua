## v0.1.1

- Fixed a huge bug with scope and some missing control flow logic with `if` statements
- Reworked numeric `for` statements with new OpCode to properly scope the iterator so upvalues close at the end of each iteration. Generic `for` still WIP!
- test coverage baked into the lib test modules now. Covers some basic and some complex features including closures, but more is still needed

## v0.1

- First release!
- Basic lua syntax compliance
- Stack based, reference counted memory management
