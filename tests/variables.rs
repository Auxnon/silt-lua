use crate::{};
use silt_lua::{ExVal,simple, test_number, test_string, valeq, test_nil, test_bool};

test_number!(local_variable, "local x = 42; return x", 42.0);
test_string!(
    local_string_var,
    "local name = 'hello'; return name",
    "hello"
);
test_bool!(local_boolean_var, "local flag = true; return flag", true);
test_nil!(local_nil_var, "local empty; return empty");

test_number!(global_variable, "x = 100; return x", 100.0);
test_string!(global_string, "message = 'world'; return message", "world");

test_number!(variable_reassignment, "local x = 5; x = 10; return x", 10.0);
test_number!(multiple_assignment, "local a, b = 1, 2; return a + b", 3.0);

#[test]
fn variable_scope() {
    let source = r#"
        local x = 1
        do
            local x = 2
            y = x
        end
        return x + y
    "#;
    valeq!(source, ExVal::Number(3.0));
}

#[test]
fn nested_scope() {
    let source = r#"
        local a = 1
        do
            local b = 2
            do
                local c = 3
                result = a + b + c
            end
        end
        return result
    "#;
    valeq!(source, ExVal::Number(6.0));
}

#[test]
fn variable_shadowing() {
    let source = r#"
        local x = 'outer'
        do
            local x = 'inner'
            inner_x = x
        end
        return x
    "#;
    valeq!(source, ExVal::String("outer".to_string()));
}

test_number!(
    arithmetic_with_variables,
    "local a = 5; local b = 3; return a * b + 2",
    17.0
);
test_string!(
    string_concatenation_vars,
    "local first = 'Hello'; local second = 'World'; return first .. ' ' .. second",
    "Hello World"
);
