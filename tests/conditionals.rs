use silt_lua::{test_number, test_string, valeq,ExVal,simple};

test_number!(simple_if_true, "if true then return 1 end; return 0", 1.0);
test_number!(simple_if_false, "if false then return 1 end; return 0", 0.0);

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

test_number!(
    if_elseif_else,
    "local x = 2; if x == 1 then return 10 elseif x == 2 then return 20 else return 30 end",
    20.0
);

test_number!(
    nested_if,
    "if true then if true then return 42 end end; return 0",
    42.0
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
fn truthiness() {
    valeq!(
        "if 0 then return true else return false end",
        ExVal::Bool(true)
    );
    valeq!(
        "if '' then return true else return false end",
        ExVal::Bool(true)
    );
    valeq!(
        "if nil then return true else return false end",
        ExVal::Bool(false)
    );
    valeq!(
        "if false then return true else return false end",
        ExVal::Bool(false)
    );
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
        ExVal::Number(15.0)
    );
}
