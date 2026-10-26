//! The `snippet` type (cargo feature `typing`): a function that may leave the VM as a
//! snippet. The checker decides per function literal whether it captures/references
//! only values, and a host declares which natives take one. Run with:
//!   cargo test --features typing --test typing_snippets
#![cfg(feature = "typing")]

use silt_lua::{Compiler, ExVal, Lua};

/// A host that exposes `instr(id, voice)` taking a snippet, and provides `emit`
/// inside snippet VMs. `instr`/`emit` are declared only — never registered — so a
/// script that calls them fails at run time; these tests compile only.
fn host_compiler() -> Compiler {
    let mut c = Compiler::new();
    c.declare("instr", "(number, snippet(number, number) -> number) -> nil")
        .unwrap();
    c.declare_in_snippet("emit", "(number) -> nil").unwrap();
    c.declare("host_only", "(number) -> number").unwrap();
    c
}

fn compile(src: &str) -> Result<(), String> {
    let mut lua = Lua::new_with_standard();
    let mut c = host_compiler();
    lua.compile(None, src, &mut c).map(|_| ()).map_err(|e| e.to_string())
}

fn run(src: &str) -> Result<ExVal, String> {
    let mut lua = Lua::new_with_standard();
    let mut c = host_compiler();
    lua.run(None, src, &mut c).map_err(|e| e.to_string())
}

fn ok(src: &str) {
    if let Err(e) = compile(src) {
        panic!("expected a clean compile, got: {e}\n{src}");
    }
}

fn type_error(src: &str, needle: &str) {
    match compile(src) {
        Err(e) => assert!(
            e.contains("TypeError") && e.contains(needle),
            "expected a TypeError containing {needle:?}, got: {e}\n{src}"
        ),
        Ok(()) => panic!("expected a type error containing {needle:?}, but it compiled\n{src}"),
    }
}

// ------------------------------------------------------ the declared boundary

#[test]
fn value_captures_and_stdlib_references_are_snippets() {
    ok("local gain = 0.3 instr(1, function(p, f) return math.sin(p * f) * gain end)");
    ok("local name = 'saw' instr(1, function(p, f) print(name) return p end)");
    ok("instr(1, function(p, f) return p end)");
}

#[test]
fn capturing_a_function_is_rejected_with_the_name() {
    // (a root-level `local function` compiles as a global in silt — see the global
    // tests — so a captured function is a typed local here)
    type_error(
        "local helper: (number) -> number = function(x) return x * 2 end
         instr(1, function(p, f) return helper(p) end)",
        "captures 'helper'",
    );
    type_error(
        "local helper: (number) -> number = function(x) return x * 2 end
         instr(1, function(p, f) return helper(p) end)",
        "could not be converted into 'snippet(number, number) -> number'",
    );
    // inside a function, `local function` is a real local
    type_error(
        "local function setup()
             local function helper(x) return x * 2 end
             instr(1, function(p, f) return helper(p) end)
         end",
        "captures 'helper'",
    );
    // the capture is named even when annotated rather than defined as a function
    type_error(
        "local cb: (number) -> number = function(x) return x end
         instr(1, function(p, f) return cb(p) end)",
        "captures 'cb'",
    );
}

#[test]
fn capturing_a_table_known_to_hold_a_function_is_rejected() {
    type_error(
        "type Osc = {gain: number, shape: (number) -> number}
         local osc: Osc = {gain = 1, shape = function(x) return x end}
         instr(1, function(p, f) return osc.shape(p) * osc.gain end)",
        "captures 'osc'",
    );
    // a pure-data shape travels by value
    ok(
        "type Cfg = {gain: number, detune: number}
         local cfg: Cfg = {gain = 1, detune = 0}
         instr(1, function(p, f) return p * cfg.gain + cfg.detune end)",
    );
}

#[test]
fn referencing_a_script_global_function_is_rejected() {
    type_error(
        "function helper(x) return x * 2 end
         instr(1, function(p, f) return helper(p) end)",
        "references global 'helper', a function of this script",
    );
    // …even from a function nested inside the snippet
    type_error(
        "function helper(x) return x * 2 end
         instr(1, function(p, f) local g = function() return helper(p) end return g() end)",
        "references global 'helper'",
    );
}

#[test]
fn host_natives_are_fine_only_when_provided_to_snippets() {
    ok("instr(1, function(p, f) emit(p) return p end)");
    type_error(
        "instr(1, function(p, f) return host_only(p) end)",
        "references global 'host_only', a host function not provided to snippets",
    );
    // the consuming native itself is not available inside the snippet
    type_error(
        "instr(1, function(p, f) instr(2, function(a, b) return a end) return p end)",
        "references global 'instr'",
    );
}

#[test]
fn unknown_globals_stay_gradual() {
    // nothing is known about `mystery`: it may well be provided by the snippet VM
    ok("instr(1, function(p, f) return mystery(p) end)");
    ok("instr(1, function(p, f) counter = (counter or 0) + 1 return p end)");
}

#[test]
fn nested_capture_through_the_snippet_is_rejected() {
    // the inner function's capture of `helper` passes through the snippet's own
    // upvalue list, so the snippet is the one holding a reference
    type_error(
        "local helper: (number) -> number = function(x) return x end
         instr(1, function(p, f)
             local inner = function() return helper(p) end
             return inner()
         end)",
        "captures 'helper'",
    );
    // captures of the snippet's *own* locals are internal and fine
    ok(
        "instr(1, function(p, f)
             local k = p * 2
             local inner = function() return k end
             return inner()
         end)",
    );
}

#[test]
fn signature_is_still_checked() {
    type_error(
        "instr(1, function(p: string, f) return 1 end)",
        "could not be converted into 'snippet(number, number) -> number'",
    );
    type_error(
        "instr(1, function(p, f): string return 'x' end)",
        "could not be converted into 'snippet(number, number) -> number'",
    );
    ok("instr(1, function(p: number, f: number): number return p + f end)");
}

// ------------------------------------------------------------ as a type

#[test]
fn snippet_annotated_locals() {
    ok("local v: snippet = function(p) return p end");
    ok("local v: snippet(number) -> number = function(p: number): number return p end");
    type_error(
        "local helper: () -> nil = function() end
         local v: snippet = function(p) return helper() end",
        "captures 'helper'",
    );
    type_error(
        "local v: snippet(number) -> number = function(a, b) return 1 end",
        "could not be converted into 'snippet(number) -> number'",
    );
}

#[test]
fn a_snippet_is_a_function() {
    ok("local v: snippet = function(p) return p end local f: function = v");
    ok(
        "local v: snippet(number) -> number = function(p: number): number return p end
         local f: (number) -> number = v
         local n: number = v(1)",
    );
    // but a value only known to be a function is accepted gradually where a snippet is wanted
    ok("local f: function = function() end local v: snippet = f");
    ok("local f: (number) -> number = function(p: number): number return p end local v: snippet(number) -> number = f");
}

#[test]
fn ordinary_functions_are_unaffected() {
    // a function that could never be a snippet is still a perfectly good function
    ok(
        "local function helper(x) return x end
         local function user(p) return helper(p) end
         local f: (number) -> any = user
         return user(1)",
    );
    assert_eq!(
        run("local function helper(x) return x * 3 end local function user(p) return helper(p) end return user(2)"),
        Ok(ExVal::Integer(6))
    );
}

// -------------------------------------------------------------- declarations

#[test]
fn declarations_persist_across_compiles() {
    let mut lua = Lua::new_with_standard();
    let mut c = host_compiler();
    lua.compile(None, "local x = 1", &mut c).map_err(|e| e.to_string()).unwrap();
    // second compile with the same compiler still knows `instr`
    let err = lua
        .compile(
            None,
            "local function h() end instr(1, function(p, f) return h() end)",
            &mut c,
        )
        .map_err(|e| e.to_string())
        .err()
        .expect("declaration should survive the compiler reset");
    assert!(err.contains("'h'") && err.contains("snippet"), "{err}");
}

#[test]
fn declared_types_are_used_as_ordinary_signatures_too() {
    type_error("instr('one', function(p, f) return p end)", "(argument 1)");
    type_error("local s: string = host_only(1)", "could not be converted into 'string'");
}

#[test]
fn bad_declarations_are_reported() {
    let mut c = Compiler::new();
    assert!(c.declare("f", "(number) ->").is_err());
    assert!(c.declare("f", "snippet(number)").is_err()); // needs `-> R`
    assert!(c.declare("f", "number string").is_err());
    assert!(c.declare("f", "snippet").is_ok());
    assert!(c.declare("f", "snippet(number, number) -> number").is_ok());
    // a failed declaration leaves the compiler usable
    let mut lua = Lua::new_with_standard();
    assert!(lua.compile(None, "return 1", &mut c).is_ok());
}

#[test]
fn std_globals_match_the_runtime() {
    let mut lua = Lua::new_with_standard();
    let mut names: Vec<String> = Vec::new();
    lua.enter(|vm, _mc| {
        for (k, _) in vm.globals.borrow().iter() {
            if let silt_lua::value::Value::String(s) = k {
                names.push(s.to_string());
            }
        }
    });
    names.sort();
    let mut listed: Vec<String> = silt_lua::lua::STANDARD_GLOBALS
        .iter()
        .map(|s| s.to_string())
        .collect();
    listed.sort();
    assert_eq!(names, listed);
}
