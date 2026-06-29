// Metamethods / OOP. `setmetatable`/`getmetatable`, table-valued `__index` (incl.
// inheritance chains and method dispatch), and the arithmetic metamethods are wired.
// Comparison (`__eq`/`__lt`/`__le`), `__concat`, `__unm`, `__newindex`, and
// function-valued `__index` are tracked below with #[ignore]; remove as each lands.
use silt_lua::{simple, valeq, ExVal};

#[test]
fn index_metamethod() {
    valeq!(
        r#"
        local base = { greet = function() return 7 end }
        local t = setmetatable({}, { __index = base })
        return t.greet()
    "#,
        ExVal::Integer(7)
    );
}

#[test]
fn add_metamethod() {
    valeq!(
        r#"
        local mt = { __add = function(a, b) return a.v + b.v end }
        local x = setmetatable({ v = 3 }, mt)
        local y = setmetatable({ v = 4 }, mt)
        return x + y
    "#,
        ExVal::Integer(7)
    );
}

#[test]
fn oop_method_dispatch() {
    valeq!(
        r#"
        local Account = {}
        Account.__index = Account
        function Account.new(balance)
            return setmetatable({ balance = balance }, Account)
        end
        function Account:deposit(n)
            self.balance = self.balance + n
        end
        local a = Account.new(100)
        a:deposit(50)
        return a.balance
    "#,
        ExVal::Integer(150)
    );
}

#[test]
fn index_inheritance_chain() {
    // Derived's metatable's __index is Base, so a lookup misses on the instance,
    // misses on Derived, and resolves on Base — multi-level inheritance.
    valeq!(
        r#"
        local Base = {}
        Base.__index = Base
        function Base:kind() return 11 end
        local Derived = setmetatable({}, { __index = Base })
        Derived.__index = Derived
        local o = setmetatable({}, Derived)
        return o:kind()
    "#,
        ExVal::Integer(11)
    );
}

#[test]
fn arithmetic_metamethods() {
    valeq!("local m={__sub=function(a,b) return a.v-b.v end} local x=setmetatable({v=9},m) local y=setmetatable({v=4},m) return x-y", ExVal::Integer(5));
    valeq!("local m={__mul=function(a,b) return a.v*b.v end} local x=setmetatable({v=3},m) local y=setmetatable({v=4},m) return x*y", ExVal::Integer(12));
}

#[test]
fn setmetatable_returns_the_table() {
    // `local t = setmetatable({}, mt)` — the idiomatic constructor relies on the
    // table being returned, not nil.
    valeq!("local t = setmetatable({x = 5}, {}) return t.x", ExVal::Integer(5));
}

#[test]
#[ignore = "metamethod: __eq/__lt/__le comparison dispatch not wired"]
fn comparison_metamethods() {
    valeq!("local m={__lt=function(a,b) return a.v<b.v end} local x=setmetatable({v=3},m) local y=setmetatable({v=4},m) return x<y", ExVal::Bool(true));
}

#[test]
#[ignore = "metamethod: __concat not wired"]
fn concat_metamethod() {
    valeq!(r#"local m={__concat=function(a,b) return "z" end} local x=setmetatable({},m) return x.."!""#, ExVal::String("z".to_string()));
}

#[test]
#[ignore = "metamethod: __newindex not wired"]
fn newindex_metamethod() {
    valeq!(
        "local store={} local t=setmetatable({},{__newindex=function(tb,k,v) store[k]=v end}) t.x=42 return store.x",
        ExVal::Integer(42)
    );
}

#[test]
#[ignore = "metamethod: function-valued __index not wired (table-valued works)"]
fn index_function_metamethod() {
    valeq!(
        r#"local t=setmetatable({},{__index=function(tb,k) return 99 end}) return t.anything"#,
        ExVal::Integer(99)
    );
}
