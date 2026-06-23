// Metamethods / OOP. Metatables are WIP (README). `setmetatable`/`getmetatable` are
// registered, but the metamethod dispatch (__index, __add, ...) and method-call sugar are
// not reliable yet. These encode the target behavior — see PLAN.md §3 (and §1.2 for the
// `function t:m()` definition crash). Remove #[ignore] as each lands.
use silt_lua::{simple, valeq, ExVal};

#[test]
#[ignore = "PLAN.md §1.2/§3 — method dispatch via __index not wired"]
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
#[ignore = "PLAN.md §3 — __add arithmetic metamethod not wired"]
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
#[ignore = "PLAN.md §1.2 — `function t:method()` definition currently panics the compiler"]
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
