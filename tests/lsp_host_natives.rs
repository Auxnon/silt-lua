//! A host's natives reach hover, completion and signature help through a `---@meta`
//! stub added to the `Library` (Petrichor64 generates one, `ignore.lua`).
#![cfg(feature = "lsp")]

use silt_lua::lsp::{completion_in, hover_in, signature_help_in, Library, Position};

const STUB: &str = "---@meta
---Spawn an entity from an asset at a position.
---@param asset string
---@param x number
---@param y number
---@param z number
---@return userdata
function make(asset, x, y, z) end
";

fn lib() -> Library {
    let mut lib = Library::default();
    lib.add("file:///app/ignore.lua", STUB);
    lib
}

#[test]
fn hover_shows_a_stub_natives_doc() {
    let src = "local e = make('crate', 1, 2, 3)";
    let h = hover_in(src, "file:///app/main.lua", Position { line: 0, character: 11 }, &lib())
        .expect("hover over make");
    assert!(h.contains("Spawn an entity"), "{h}");
}

#[test]
fn completion_offers_stub_natives() {
    let src = "ma";
    let items = completion_in(src, "file:///app/main.lua", Position { line: 0, character: 2 }, &lib());
    assert!(items.iter().any(|c| c.label == "make"), "{:?}", items.iter().map(|c| &c.label).collect::<Vec<_>>());
}

#[test]
fn signature_help_for_stub_natives() {
    let src = "make('crate', ";
    let sig = signature_help_in(src, "file:///app/main.lua", Position { line: 0, character: 14 }, &lib())
        .expect("signature help inside make(");
    assert!(sig.label.contains("asset") && sig.label.contains("z"), "{}", sig.label);
}
