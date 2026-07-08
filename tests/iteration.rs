// Generic `for … in` plus the iteration builtins next / pairs / ipairs, built on
// the native multi-return ABI. See PLAN.md §2.7 / §3.
use silt_lua::{simple, valeq, ExVal};

#[test]
fn ipairs_values_and_indices() {
    valeq!("local s=0 for i,v in ipairs({10,20,30}) do s=s+v end return s", ExVal::Integer(60));
    valeq!("local s=0 for i,v in ipairs({5,6,7}) do s=s+i end return s", ExVal::Integer(6));
}

#[test]
fn pairs_sum() {
    valeq!(
        "local t={a=1,b=2,c=3} local s=0 for k,v in pairs(t) do s=s+v end return s",
        ExVal::Integer(6)
    );
}

#[test]
fn pairs_single_var_keys() {
    valeq!("local c=0 for k in pairs({a=1,b=2}) do c=c+1 end return c", ExVal::Integer(2));
}

#[test]
fn empty_tables() {
    valeq!("local n=0 for k,v in pairs({}) do n=n+1 end return n", ExVal::Integer(0));
    valeq!("local n=0 for i,v in ipairs({}) do n=n+1 end return n", ExVal::Integer(0));
}

#[test]
fn next_direct() {
    valeq!("local t={only=42} local k,v = next(t) return v", ExVal::Integer(42));
    valeq!("return next({})", ExVal::Nil);
}

#[test]
fn break_inside_generic_for() {
    valeq!(
        "local s=0 for i,v in ipairs({1,2,3,4}) do if v==3 then break end s=s+v end return s",
        ExVal::Integer(3)
    );
}

#[test]
fn body_local_in_generic_for() {
    valeq!(
        "local s=0 for i,v in ipairs({1,2,3}) do local d=v*10 s=s+d end return s",
        ExVal::Integer(60)
    );
}

#[test]
fn nested_generic_for() {
    valeq!(
        "local s=0 for i,a in ipairs({1,2}) do for j,b in ipairs({10,20}) do s=s+a*b end end return s",
        ExVal::Integer(90)
    );
}

#[test]
fn ipairs_stops_at_first_nil() {
    // ipairs only walks the contiguous 1..n sequence
    valeq!(
        "local t={} t[1]=1 t[2]=2 t[4]=4 local n=0 for i,v in ipairs(t) do n=n+1 end return n",
        ExVal::Integer(2)
    );
}
