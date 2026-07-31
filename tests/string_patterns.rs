use silt_lua::{simple, valeq, ExVal};

// Lua pattern matching: string.find / string.match / string.gsub.
// The engine lives in src/lua_pattern.rs (a port of lstrlib.c). These cover the pattern
// language (classes, sets, anchors, quantifiers, captures, %b, %f) and each function's
// return contract, including multi-value returns.

// --- find -------------------------------------------------------------------

#[test]
fn find_plain_and_pattern() {
    valeq!(r#"return string.find("hello world", "world")"#, ExVal::Integer(7));
    valeq!(
        r#"local s,e = string.find("hello world","world") return s*100+e"#,
        ExVal::Integer(711)
    );
    // plain=true makes magic chars literal
    valeq!(r#"return string.find("a.b.c", ".", 1, true)"#, ExVal::Integer(2));
    valeq!(r#"return string.find("hello", "xyz") == nil"#, ExVal::Bool(true));
}

#[test]
fn find_with_captures() {
    valeq!(
        r#"local s,e,k,v = string.find("key=val","(%w+)=(%w+)")
           return s.."/"..e.."/"..k.."/"..v"#,
        ExVal::String("1/7/key/val".to_string())
    );
}

#[test]
fn find_init_negative() {
    // search starting near the end
    valeq!(r#"return string.find("aXbXc", "X", -3)"#, ExVal::Integer(4));
}

// --- match ------------------------------------------------------------------

#[test]
fn match_whole_and_captures() {
    valeq!(r#"return string.match("hello123", "%d+")"#, ExVal::String("123".to_string()));
    valeq!(
        r#"local y,m,d = string.match("2026-07-16","(%d+)-(%d+)-(%d+)")
           return y.."/"..m.."/"..d"#,
        ExVal::String("2026/07/16".to_string())
    );
    valeq!(r#"return string.match("nope", "%d+") == nil"#, ExVal::Bool(true));
}

#[test]
fn match_trim_idiom() {
    valeq!(
        r#"return string.match("  trim me  ", "^%s*(.-)%s*$")"#,
        ExVal::String("trim me".to_string())
    );
}

#[test]
fn match_position_capture() {
    valeq!(
        r#"local a,b = string.match("hello","()ll()") return a*10+b"#,
        ExVal::Integer(35)
    );
}

// --- character classes & sets ----------------------------------------------

#[test]
fn classes_and_sets() {
    valeq!(r#"return string.match("  abc123", "[%a]+")"#, ExVal::String("abc".to_string()));
    valeq!(r#"return string.match("foo_bar-baz", "[%w_]+")"#, ExVal::String("foo_bar".to_string()));
    valeq!(r#"return string.match("a1b2", "[^%d]")"#, ExVal::String("a".to_string()));
    valeq!(r#"return string.match("HELLOworld", "%u+")"#, ExVal::String("HELLO".to_string()));
}

// --- gsub: string / table / function replacement ---------------------------

#[test]
fn gsub_string_repl() {
    valeq!(
        r#"local r = string.gsub("hello world", "o", "0") return r"#,
        ExVal::String("hell0 w0rld".to_string())
    );
    // count is the second return
    valeq!(r#"local _,n = string.gsub("hello", "l", "L") return n"#, ExVal::Integer(2));
    // limited to n replacements
    valeq!(
        r#"local r = string.gsub("hello", "l", "L", 1) return r"#,
        ExVal::String("heLlo".to_string())
    );
    // back-references
    valeq!(
        r#"local r = string.gsub("hello world", "(%w+)", "[%1]") return r"#,
        ExVal::String("[hello] [world]".to_string())
    );
    valeq!(r#"local r = string.gsub("abc", "%w", "%0%0") return r"#, ExVal::String("aabbcc".to_string()));
}

#[test]
fn gsub_table_repl() {
    valeq!(
        r#"local t = {name="Sam", place="home"}
           local r = string.gsub("$name is $place", "%$(%w+)", t) return r"#,
        ExVal::String("Sam is home".to_string())
    );
    // a key absent from the table keeps the original match
    valeq!(
        r#"local t = {a="X"} local r = string.gsub("a b", "%w", t) return r"#,
        ExVal::String("X b".to_string())
    );
}

#[test]
fn gsub_function_repl() {
    valeq!(
        r#"local r = string.gsub("abc", "%w", function(c) return c:upper() end) return r"#,
        ExVal::String("ABC".to_string())
    );
    valeq!(
        r#"local r = string.gsub("1 2 3", "%d", function(d) return tostring(tonumber(d)*2) end) return r"#,
        ExVal::String("2 4 6".to_string())
    );
    // returning nil keeps the original match
    valeq!(
        r#"local r = string.gsub("abc", "%w", function() return nil end) return r"#,
        ExVal::String("abc".to_string())
    );
}

#[test]
fn gsub_anchored_and_empty() {
    valeq!(r#"local r = string.gsub("aaa", "^a", "X") return r"#, ExVal::String("Xaa".to_string()));
    valeq!(r#"local r = string.gsub("hi", "", "-") return r"#, ExVal::String("-h-i-".to_string()));
}

// --- balanced / frontier ----------------------------------------------------

#[test]
fn balanced_match() {
    valeq!(
        r#"return string.match("(a(b)c) rest", "%b()")"#,
        ExVal::String("(a(b)c)".to_string())
    );
}

#[test]
fn frontier_pattern() {
    valeq!(
        r#"local r = string.gsub("THE quick fox", "%f[%a]%a+", function(w) return "<"..w..">" end)
           return r"#,
        ExVal::String("<THE> <quick> <fox>".to_string())
    );
}

// --- method-call sugar dispatches through the string metatable -------------

#[test]
fn method_call_sugar() {
    valeq!(r#"return ("hello"):match("l+")"#, ExVal::String("ll".to_string()));
    valeq!(r#"return ("a,b,c"):gsub(",", ";")"#, ExVal::String("a;b;c".to_string()));
}
