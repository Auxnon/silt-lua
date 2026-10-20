//! Editor features of the LSP analysis core (feature `lsp`): hover, go-to-
//! definition, references/rename, completion, document symbols, signature
//! help, semantic tokens, and full-span diagnostics. Run with:
//!   cargo test --features lsp --test lsp_features
#![cfg(feature = "lsp")]

use silt_lua::lsp::{
    completion, completion_in, definition, definition_in, diagnostics, document_symbols, hover,
    hover_in, references, semantic_tokens, signature_help, signature_help_in, CompletionKind,
    Library, Position, SymbolKind,
};

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

const SRC: &str = "\
-- Adds two numbers.
local function add(a: number, b: number): number
    return a + b
end
local total = add(1, 2)
local t = {x = 1}
function t.scale(k) return t.x * k end
function t:bump() self.x = self.x + 1 end
type Vec = {x: number, y: number}
local v: Vec = {x = 0, y = 0}
print(total, v.x)
";

#[test]
fn diagnostics_have_full_spans() {
    // `1 + {}` is a runtime error, but `if x then` unterminated is a compile error
    // whose span the compiler records.
    let d = diagnostics("local a = 1\nif a then\n  a = 2");
    assert!(!d.is_empty());
    assert!(d[0].range.end >= d[0].range.start);
}

#[test]
fn hover_on_function_shows_signature_and_doc() {
    let h = hover(SRC, pos(1, 16)).expect("hover on `add`");
    assert!(
        h.contains("local function add(a: number, b: number): number"),
        "{h}"
    );
    assert!(h.contains("Adds two numbers."), "{h}");
    // at a use site too
    let h2 = hover(SRC, pos(4, 15)).expect("hover on `add` call");
    assert!(h2.contains("function add("), "{h2}");
}

#[test]
fn hover_on_locals_and_params() {
    let h = hover(SRC, pos(2, 11)).expect("hover on param `a`");
    assert!(h.contains("(parameter) a: number"), "{h}");
    let h = hover(SRC, pos(9, 7)).expect("hover on `v`");
    assert!(h.contains("local v: Vec"), "{h}");
    let h = hover(SRC, pos(8, 5)).expect("hover on alias `Vec`");
    assert!(h.contains("type Vec = {x: number, y: number}"), "{h}");
}

#[test]
fn hover_on_stdlib_global() {
    let h = hover(SRC, pos(10, 1)).expect("hover on print");
    assert!(h.contains("print") && h.contains("standard library"), "{h}");
}

#[test]
fn go_to_definition() {
    // `add` used on line 4 is defined on line 1
    let d = definition(SRC, pos(4, 15)).expect("definition of add");
    assert_eq!((d.start.line, d.start.character), (1, 15));
    // `total` used in print → its local
    let d = definition(SRC, pos(10, 7)).expect("definition of total");
    assert_eq!(d.start.line, 4);
    // a param used in the body resolves to the param
    let d = definition(SRC, pos(2, 11)).expect("definition of a");
    assert_eq!((d.start.line, d.start.character), (1, 19));
}

#[test]
fn references_include_definition_and_uses() {
    let refs = references(SRC, pos(1, 16));
    let lines: Vec<u32> = refs.iter().map(|r| r.start.line).collect();
    assert_eq!(lines, vec![1, 4], "{refs:?}");
    let refs = references(SRC, pos(4, 7)); // total
    assert_eq!(refs.len(), 2);
}

#[test]
fn shadowing_resolves_to_the_innermost_definition() {
    let src = "local x = 1\ndo\n  local x = 2\n  print(x)\nend\nprint(x)";
    let inner = definition(src, pos(3, 8)).unwrap();
    assert_eq!(inner.start.line, 2);
    let outer = definition(src, pos(5, 6)).unwrap();
    assert_eq!(outer.start.line, 0);
}

#[test]
fn loop_variables_and_block_scopes() {
    let src = "for i = 1, 3 do\n  print(i)\nend\nprint(i)";
    let d = definition(src, pos(1, 8)).unwrap();
    assert_eq!(d.start.line, 0);
    // after the loop, `i` is not the loop variable
    assert!(definition(src, pos(3, 6)).is_none());
}

#[test]
fn completion_in_scope_and_keywords() {
    let src = "local alpha = 1\nlocal function apply() end\nap";
    let items = completion(src, pos(2, 2));
    let labels: Vec<&str> = items.iter().map(|c| c.label.as_str()).collect();
    assert!(labels.contains(&"apply"), "{labels:?}");
    assert!(!labels.contains(&"alpha"), "{labels:?}");
    let kw = completion("re", pos(0, 2));
    assert!(kw
        .iter()
        .any(|c| c.label == "return" && c.kind == CompletionKind::Keyword));
}

#[test]
fn completion_of_members_and_stdlib() {
    let items = completion(SRC, pos(11, 0)); // end of file: everything visible
    assert!(items.iter().any(|c| c.label == "print"));
    assert!(items
        .iter()
        .any(|c| c.label == "string" && c.kind == CompletionKind::Module));
    // members seen on `t`
    let src = format!("{}t.", SRC);
    let members = completion(&src, pos(11, 2));
    let labels: Vec<&str> = members.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"scale") && labels.contains(&"x") && labels.contains(&"bump"),
        "{labels:?}"
    );
    // stdlib table members
    let m = completion("string.", pos(0, 7));
    assert!(
        m.iter().any(|c| c.label == "upper"),
        "{:?}",
        m.iter().map(|c| &c.label).collect::<Vec<_>>()
    );
    // typed receiver: declared shape fields
    let m = completion(&format!("{}v.", SRC), pos(11, 2));
    assert!(m.iter().any(|c| c.label == "y"));
}

#[test]
fn completion_of_type_names_after_colon() {
    let items = completion(&format!("{}local q: ", SRC), pos(11, 9));
    let labels: Vec<&str> = items.iter().map(|c| c.label.as_str()).collect();
    assert!(
        labels.contains(&"number") && labels.contains(&"Vec"),
        "{labels:?}"
    );
    assert!(!labels.contains(&"print"));
}

#[test]
fn document_symbol_outline() {
    let syms = document_symbols(SRC);
    let names: Vec<String> = syms.iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"add".to_string()));
    assert!(names.contains(&"t.scale".to_string()));
    assert!(names.contains(&"t:bump".to_string()));
    assert!(names.contains(&"Vec".to_string()));
    assert!(names.contains(&"total".to_string()));
    assert!(syms
        .iter()
        .any(|s| s.name == "add" && s.kind == SymbolKind::LocalFunction));
    // parameters are not outline entries
    assert!(!names.contains(&"a".to_string()));
}

#[test]
fn signature_help_tracks_active_parameter() {
    let src = format!("{}add(1, ", SRC);
    let sh = signature_help(&src, pos(11, 7)).expect("signature help");
    assert_eq!(sh.label, "add(a: number, b: number)");
    assert_eq!(sh.active_parameter, 1);
    let src = format!("{}add(", SRC);
    let sh = signature_help(&src, pos(11, 4)).unwrap();
    assert_eq!(sh.active_parameter, 0);
    // method call drops the implicit self
    let src = format!("{}t:bump(", SRC);
    let sh = signature_help(&src, pos(11, 7)).unwrap();
    assert_eq!(sh.label, "bump()");
}

#[test]
fn semantic_tokens_classify_tokens() {
    let toks = semantic_tokens("local function f(a) return a + 1 end");
    // keyword, keyword, function, parameter, keyword, parameter, operator, number, keyword
    let kinds: Vec<u32> = toks.iter().map(|t| t.3).collect();
    let legend = silt_lua::lsp::SEMANTIC_TOKEN_TYPES;
    let names: Vec<&str> = kinds.iter().map(|k| legend[*k as usize]).collect();
    assert_eq!(
        names,
        vec![
            "keyword",
            "keyword",
            "function",
            "parameter",
            "keyword",
            "parameter",
            "operator",
            "number",
            "keyword"
        ],
    );
}

// A `---@meta` stub in the lua-language-server style (like Petrichor64's generated
// `ignore.lua`), loaded as a library for another document.
const STUB: &str = "\
---@meta

-- Sine value
---@param f number
---@return number
function sin(f)
end

--- @type number ~3.1457
pi = nil

-- Spawn an entity
---@param asset string model asset
---@param x number
function make(asset, x)
end
";

fn stub_library() -> Library {
    let mut lib = Library::default();
    lib.add("file:///proj/scripts/ignore.lua", STUB);
    lib
}

#[test]
fn library_hover_folds_annotations_into_signature() {
    let lib = stub_library();
    let src = "local y = sin(1)\nprint(pi)\nmake(\"a\", 1)";
    let h = hover_in(src, "file:///proj/scripts/main.lua", pos(0, 11), &lib).unwrap();
    assert!(h.contains("function sin(f: number): number"), "{h}");
    assert!(h.contains("Sine value"), "{h}");
    assert!(!h.contains("@param"), "{h}");
    assert!(h.contains("ignore.lua"), "{h}");
    let h = hover_in(src, "file:///proj/scripts/main.lua", pos(1, 7), &lib).unwrap();
    assert!(h.contains("pi") && h.contains("~3.1457"), "{h}");
    let h = hover_in(src, "file:///proj/scripts/main.lua", pos(2, 1), &lib).unwrap();
    assert!(h.contains("function make(asset: string, x: number)"), "{h}");
    assert!(h.contains("`asset` — model asset"), "{h}");
}

#[test]
fn library_definition_completion_and_signature() {
    let lib = stub_library();
    let uri = "file:///proj/scripts/main.lua";
    let (def_uri, r) = definition_in("sin(1)", uri, pos(0, 1), &lib).unwrap();
    assert_eq!(def_uri, "file:///proj/scripts/ignore.lua");
    assert_eq!(r.start.line, 5);
    // a document's own definition wins over the library's
    let (def_uri, _) = definition_in("function sin(a) end\nsin(1)", uri, pos(1, 1), &lib).unwrap();
    assert_eq!(def_uri, uri);
    let items = completion_in("ma", uri, pos(0, 2), &lib);
    assert!(items.iter().any(|c| c.label == "make"));
    let sh = signature_help_in("make(\"a\", ", uri, pos(0, 10), &lib).unwrap();
    assert_eq!(sh.label, "make(asset: string, x: number)");
    assert_eq!(sh.active_parameter, 1);
    // the library file itself isn't its own library
    assert!(hover_in("sin(1)", "file:///proj/scripts/ignore.lua", pos(0, 1), &lib).is_none());
}

#[test]
fn library_uris_and_paths_name_the_same_file() {
    let mut lib = Library::default();
    lib.add("/proj/my scripts/a.lua", "function helper() end");
    assert!(lib.contains("file:///proj/my%20scripts/a.lua"));
    let (def_uri, _) = definition_in("helper()", "file:///proj/b.lua", pos(0, 1), &lib).unwrap();
    assert_eq!(def_uri, "file:///proj/my%20scripts/a.lua");
}

#[cfg(feature = "vector")]
#[test]
fn vector_methods_and_constructors_are_documented() {
    let src = "local a = vec3(1, 0, 0)\nlocal b = a:dot(a)\nlocal c = a:cross(a)\n";
    let h = hover(src, pos(1, 13)).unwrap();
    assert!(h.contains("function vec:dot(other: vec): number"), "{h}");
    let h = hover(src, pos(0, 12)).unwrap();
    assert!(h.contains("function vec3(x: number, y: number, z: number): vec3"), "{h}");
    let sh = signature_help("local a = vec3(1, 0, 0)\na:cross(", pos(1, 8)).unwrap();
    assert_eq!(sh.label, "cross(other: vec3)");
    let items = completion("local a = vec3(1, 0, 0)\na:", pos(1, 2));
    assert!(items.iter().any(|c| c.label == "normalize"));
}

#[test]
fn library_annotations_keep_multi_word_types_whole() {
    let mut lib = Library::default();
    lib.add(
        "/proj/ignore.lua",
        "\
-- Set a voice
---@param cfg { breath?: number, hz?: number } breath 0..1, rate Hz
---@param kind string | nil filter kind
---@return boolean|{v:number[]}|nil
function vox(cfg, kind) end
",
    );
    let h = hover_in("vox()", "/proj/main.lua", pos(0, 1), &lib).unwrap();
    assert!(
        h.contains("function vox(cfg: { breath?: number, hz?: number }, kind: string | nil): boolean|{v:number[]}|nil"),
        "{h}"
    );
    assert!(h.contains("`cfg` — breath 0..1, rate Hz"), "{h}");
}

#[test]
fn hover_infers_unannotated_types() {
    let lib = stub_library();
    let uri = "file:///proj/scripts/main.lua";
    let src = "\
local ca = cos2(1)
local s = sin(0.5) * 2
local n = 3
local name = \"a\" .. n
local ok = n > 1
local t = {1, 2}
local copy = s
local f = x -> x + 1
LATER = nil
function setup() LATER = sin(1) end
local unknown = mystery()
function cos2(a: number): number return a end
";
    let ty = |line: u32, ch: u32| hover_in(src, uri, pos(line, ch), &lib).unwrap();
    assert!(ty(0, 7).contains("local ca: number"), "{}", ty(0, 7));
    assert!(ty(1, 7).contains("local s: number"), "{}", ty(1, 7));
    assert!(ty(2, 7).contains("local n: number"));
    assert!(ty(3, 7).contains("local name: string"));
    assert!(ty(4, 7).contains("local ok: boolean"));
    assert!(ty(5, 7).contains("local t: table"));
    assert!(ty(6, 7).contains("local copy: number"), "{}", ty(6, 7));
    assert!(ty(7, 7).contains("local f: function"), "{}", ty(7, 7));
    // a global first set to nil takes its type from a later assignment
    assert!(ty(8, 1).contains("(global) LATER: number"), "{}", ty(8, 1));
    // nothing known: no type rather than a wrong one
    assert!(ty(10, 7).contains("local unknown\n"), "{}", ty(10, 7));
}

#[cfg(feature = "vector")]
#[test]
fn hover_infers_vector_types_through_methods() {
    let src = "local a = vec3(1, 0, 0)\nlocal n = a:normalize()\nlocal d = a:dot(n)\nlocal c = a:cross(n):normalize()\n";
    let h = |line: u32| hover(src, pos(line, 7)).unwrap();
    assert!(h(0).contains("local a: vec3"), "{}", h(0));
    assert!(h(1).contains("local n: vec3"), "{}", h(1));
    assert!(h(2).contains("local d: number"), "{}", h(2));
    assert!(h(3).contains("local c: vec3"), "{}", h(3));
}

#[cfg(feature = "typing")]
mod library_type_checks {
    use super::*;
    use silt_lua::lsp::diagnostics_with;

    const TYPED_STUB: &str = "\
---@param xs integer[]
---@param cb fun(x: number, y?: string): boolean
---@param cfg { breath?: number, hz: number }?
---@param kind 'low'|'high'
---@return number
---@return string
function busy(xs, cb, cfg, kind) end

-- no annotations: not declared, stays dynamic
function loose(a) end

---@param ... number
function sum(...) end
";

    fn decl(lib: &Library, name: &str) -> Option<String> {
        lib.declarations("file:///proj/main.lua")
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t)
    }

    #[test]
    fn lua_ls_annotations_translate_to_declarations() {
        let mut lib = stub_library();
        lib.add("file:///proj/typed.lua", TYPED_STUB);
        assert_eq!(decl(&lib, "sin").as_deref(), Some("(number) -> number"));
        assert_eq!(decl(&lib, "make").as_deref(), Some("(string, number) -> any"));
        assert_eq!(decl(&lib, "pi").as_deref(), Some("number"));
        assert_eq!(
            decl(&lib, "busy").as_deref(),
            Some("({number}, (number, string?) -> boolean, {breath: number?, hz: number}?, string | string) -> (number, string)")
        );
        assert_eq!(decl(&lib, "loose"), None);
        assert_eq!(decl(&lib, "sum").as_deref(), Some("(...number) -> any"));
        // a file's own globals aren't declared to it
        assert!(lib.declarations("file:///proj/typed.lua").iter().all(|(n, _)| n != "busy"));
    }

    fn check(lib: &Library, src: &str) -> Vec<String> {
        let decls = lib.declarations("file:///proj/main.lua");
        diagnostics_with(src, |c| {
            for (n, t) in &decls {
                c.declare(n, t).unwrap();
            }
        })
        .into_iter()
        .map(|d| d.message)
        .collect()
    }

    #[test]
    fn type_errors_span_the_offending_expression() {
        let lib = stub_library();
        let decls = lib.declarations("file:///proj/main.lua");
        let span = |src: &str| {
            let d = diagnostics_with(src, |c| {
                for (n, t) in &decls {
                    c.declare(n, t).unwrap();
                }
            });
            let r = d.first().expect("a type error").range;
            (r.start.line, r.start.character, r.end.line, r.end.character)
        };
        // the initializer `sin(x)`, closing paren included
        assert_eq!(span("    local ca: string = sin(cam_alt)"), (0, 23, 0, 35));
        // a bad argument underlines the call
        assert_eq!(span("local x = sin(\"hello\") + 1"), (0, 10, 0, 22));
        // a string literal starts at its quote
        assert_eq!(span("local x: number = \"hi\""), (0, 18, 0, 22));
        // the whole binary expression, across lines
        assert_eq!(span("local s: string = sin(1) +\n  sin(2)"), (0, 18, 1, 8));
    }

    #[test]
    fn library_declarations_flag_type_conflicts() {
        let lib = stub_library();
        assert!(check(&lib, "local y = sin(1)\nmake(\"a\", 2)").is_empty());
        let e = check(&lib, "sin(\"x\")");
        assert!(e.iter().any(|m| m.contains("'string' could not be converted into 'number'")), "{e:?}");
        let e = check(&lib, "local s: string = sin(1)");
        assert!(e.iter().any(|m| m.contains("'number' could not be converted into 'string'")), "{e:?}");
        let e = check(&lib, "local p: string = pi");
        assert!(!e.is_empty(), "{e:?}");
    }
}
