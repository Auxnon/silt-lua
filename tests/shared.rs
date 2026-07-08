use silt_lua::{Compiler, ExVal, Lua};


pub fn run_lua(source: &str) -> Result<ExVal, Vec<String>> {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    lua.run(None,source, &mut compiler)
        .map_err(|e| e.errors.iter().map(|err| err.to_string()).collect())
}
