//! Calling a fixed-arity function with more args than it declares drops the extras,
//! as in Lua. They used to stay on the stack where the callee's first locals belong,
//! so `local a = 1 ... return a` read the extra arg instead.

use silt_lua::{simple, valeq, ExVal};

#[test]
fn extra_args_do_not_shift_locals() {
    valeq!(r#"
        function f() local a = 1 local b = 2 return a * 10 + b end
        return f(99)
    "#, ExVal::Integer(12));
}

#[test]
fn extra_args_after_params() {
    valeq!(r#"
        function f(x) local a = 1 local b = 2 return x + a * 10 + b end
        return f(100, 7, 8)
    "#, ExVal::Integer(112));
}

#[test]
fn extra_args_through_pcall() {
    valeq!(r#"
        function f() local a = 1 local b = 2 return a * 10 + b end
        local ok, v = pcall(f, 99)
        return v
    "#, ExVal::Integer(12));
}

#[test]
fn varargs_still_see_extras() {
    valeq!(r#"
        function f(x, ...) local a = 1 return x + select('#', ...) + a end
        return f(10, 7, 8, 9)
    "#, ExVal::Integer(14));
}
