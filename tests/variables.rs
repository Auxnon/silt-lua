use silt_lua::{simple, test_bool, test_int, test_nil, test_string, valeq, ExVal};

test_int!(local_variable, "local x = 42; return x", 42);
test_string!(local_string_var, "local name = 'hello'; return name", "hello");
test_bool!(local_boolean_var, "local flag = true; return flag", true);
test_nil!(local_nil_var, "local empty; return empty");

test_int!(global_variable, "x = 100; return x", 100);
test_string!(global_string, "message = 'world'; return message", "world");

test_int!(variable_reassignment, "local x = 5; x = 10; return x", 10);
test_int!(multiple_assignment, "local a, b = 1, 2; return a + b", 3);

#[test]
fn multiple_assignment_swap() {
    valeq!("local a, b = 1, 2; a, b = b, a; return a", ExVal::Integer(2));
    valeq!("local a, b = 1, 2; a, b = b, a; return b", ExVal::Integer(1));
}

#[test]
fn variable_scope() {
    valeq!(
        r#"
        local x = 1
        do
            local x = 2
            y = x
        end
        return x + y
    "#,
        ExVal::Integer(3)
    );
}

#[test]
fn nested_scope() {
    valeq!(
        r#"
        local a = 1
        do
            local b = 2
            do
                local c = 3
                result = a + b + c
            end
        end
        return result
    "#,
        ExVal::Integer(6)
    );
}

#[test]
fn variable_shadowing() {
    valeq!(
        r#"
        local x = 'outer'
        do
            local x = 'inner'
            inner_x = x
        end
        return x
    "#,
        ExVal::String("outer".to_string())
    );
}

test_int!(
    arithmetic_with_variables,
    "local a = 5; local b = 3; return a * b + 2",
    17
);
test_string!(
    string_concatenation_vars,
    "local first = 'Hello'; local second = 'World'; return first .. ' ' .. second",
    "Hello World"
);

#[test]
fn partial_local_nil_in_function() {
    valeq!(
        r#"
        function t()
            local a, b, c = 5
            if b == nil then return 999 end
            return 0
        end
        return t()
    "#,
        ExVal::Integer(999)
    );
}
