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
#[ignore = "PLAN.md §2.3 — elseif recursion double-eats the condition's first token"]
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
