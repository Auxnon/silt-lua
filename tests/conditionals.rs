use silt_lua::{simple, test_int, test_string, valeq, ExVal};

test_int!(simple_if_true, "if true then return 1 end; return 0", 1);
test_int!(simple_if_false, "if false then return 1 end; return 0", 0);

test_string!(
    if_else_true,
    "if true then return 'yes' else return 'no' end",
    "yes"
);
test_string!(
    if_else_false,
    "if false then return 'yes' else return 'no' end",
    "no"
);

test_int!(
    nested_if,
    "if true then if true then return 42 end end; return 0",
    42
);

// Regression: a plain `if ... then ... end` (no else) must consume its own
// `end`. It previously leaked the `end` to the enclosing block, which emitted a
// stray POP that corrupted a live local declared before the `if`. Each of these
// reads a local AFTER the if-block, so a stray POP surfaces as a nil.
#[test]
fn plain_if_preserves_prior_local() {
    valeq!("local x = 5 if x == 0 then x = 1 end return x + 100", ExVal::Integer(105));
}

#[test]
fn function_if_early_return_keeps_params() {
    // `if cond then return ... end` guard followed by use of a parameter
    valeq!(
        "local function m(n) if n == 0 then return -1 end return n + 1 end return m(41)",
        ExVal::Integer(42)
    );
    valeq!(
        "local function m(n) if n == 0 then return -1 end return n + 1 end return m(0)",
        ExVal::Integer(-1)
    );
}

#[test]
fn plain_if_no_semicolon_then_statement() {
    valeq!("local x = 5 if x == 99 then x = 1 end return x", ExVal::Integer(5));
}

#[test]
fn comparison_operators() {
    valeq!("return 5 == 5", ExVal::Bool(true));
    valeq!("return 5 == 3", ExVal::Bool(false));
    valeq!("return 5 ~= 3", ExVal::Bool(true));
    valeq!("return 5 ~= 5", ExVal::Bool(false));
    valeq!("return 5 > 3", ExVal::Bool(true));
    valeq!("return 3 > 5", ExVal::Bool(false));
    valeq!("return 5 >= 5", ExVal::Bool(true));
    valeq!("return 3 >= 5", ExVal::Bool(false));
    valeq!("return 3 < 5", ExVal::Bool(true));
    valeq!("return 5 < 3", ExVal::Bool(false));
    valeq!("return 5 <= 5", ExVal::Bool(true));
    valeq!("return 5 <= 3", ExVal::Bool(false));
}

#[test]
fn comparison_mixed_int_float() {
    valeq!("return 1 == 1.0", ExVal::Bool(true));
    valeq!("return 2 < 2.5", ExVal::Bool(true));
    valeq!("return 3.0 >= 3", ExVal::Bool(true));
}

#[test]
fn logical_operators() {
    valeq!("return true and true", ExVal::Bool(true));
    valeq!("return true and false", ExVal::Bool(false));
    valeq!("return false and true", ExVal::Bool(false));
    valeq!("return true or false", ExVal::Bool(true));
    valeq!("return false or false", ExVal::Bool(false));
    valeq!("return not true", ExVal::Bool(false));
    valeq!("return not false", ExVal::Bool(true));
}

#[test]
fn logical_value_semantics() {
    // and/or return operands, not booleans, in Lua
    valeq!("return 1 and 2", ExVal::Integer(2));
    valeq!("return nil and 2", ExVal::Nil);
    valeq!("return nil or 7", ExVal::Integer(7));
    valeq!("return 5 or 7", ExVal::Integer(5));
    valeq!("return false or 'x'", ExVal::String("x".to_string()));
}

#[test]
fn truthiness() {
    // only nil and false are falsy; 0 and '' are truthy
    valeq!("if 0 then return true else return false end", ExVal::Bool(true));
    valeq!("if '' then return true else return false end", ExVal::Bool(true));
    valeq!("if nil then return true else return false end", ExVal::Bool(false));
    valeq!("if false then return true else return false end", ExVal::Bool(false));
}

#[test]
fn complex_conditions() {
    valeq!(
        r#"
        local x = 5
        local y = 10
        if x > 0 and y > 0 then
            return x + y
        else
            return 0
        end
    "#,
        ExVal::Integer(15)
    );
}

// =====================================================================================
// BROKEN — see PLAN.md
// =====================================================================================

#[test]
fn if_elseif_else() {
    valeq!(
        "local x = 2; if x == 1 then return 10 elseif x == 2 then return 20 else return 30 end",
        ExVal::Integer(20)
    );
    valeq!(
        "local x = 9; if x == 1 then return 10 elseif x == 2 then return 20 else return 30 end",
        ExVal::Integer(30)
    );
}

#[test]
fn if_elseif_chain_and_fallthrough() {
    // multiple elseif arms, and the if-taken branch must NOT fall through into
    // later arms (the FORWARD over the chain) — bodies are assignments so a
    // missing jump would overwrite the result.
    valeq!(
        "local r=0 local x=3 if x==1 then r=1 elseif x==2 then r=2 elseif x==3 then r=3 else r=4 end return r",
        ExVal::Integer(3)
    );
    valeq!(
        "local r=0 local x=1 if x==1 then r=10 elseif x==2 then r=20 else r=30 end return r",
        ExVal::Integer(10)
    );
    // elseif with no else, no branch taken -> falls through to following code
    valeq!(
        "local x=5 if x==1 then return 1 elseif x==2 then return 2 end return 99",
        ExVal::Integer(99)
    );
}
