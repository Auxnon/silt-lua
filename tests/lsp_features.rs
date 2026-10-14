//! Editor features of the LSP analysis core (feature `lsp`): hover, go-to-
//! definition, references/rename, completion, document symbols, signature
//! help, semantic tokens, and full-span diagnostics. Run with:
//!   cargo test --features lsp --test lsp_features
#![cfg(feature = "lsp")]

use silt_lua::lsp::{
    completion, definition, diagnostics, document_symbols, hover, references, semantic_tokens,
    signature_help, CompletionKind, Position, SymbolKind,
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
