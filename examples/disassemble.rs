//! Print the bytecode silt compiles a script to: `cargo run --example disassemble f.lua`
//! (pass `--implicit` to compile as the `silt` command does, with implicit returns).

use silt_lua::{Compiler, Lua};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let implicit = args.iter().any(|a| a == "--implicit");
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: disassemble [--implicit] <file.lua>");
        std::process::exit(2);
    };
    let src = std::fs::read_to_string(path).expect("read script");
    let mut compiler = if implicit {
        Compiler::new_with_flags(true, true, false)
    } else {
        Compiler::new()
    };
    if let Err(e) = Lua::new_with_standard().disassemble(Some(path), &src, &mut compiler) {
        eprintln!("{:?}", e);
        std::process::exit(1);
    }
}
