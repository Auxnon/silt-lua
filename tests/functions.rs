use silt_lua::{simple, valeq, ExVal};

#[test]
fn simple_function() {
    valeq!(
        r#"
        function add(a, b) return a + b end
        return add(3, 4)
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_local() {
    valeq!(
        r#"
        local function multiply(x, y) return x * y end
        return multiply(6, 7)
    "#,
        ExVal::Integer(42)
    );
}

#[test]
fn function_no_parameters() {
    valeq!(
        r#"
        function get_answer() return 42 end
        return get_answer()
    "#,
        ExVal::Integer(42)
    );
}

#[test]
fn function_blocked() {
    valeq!(
        r#"
        do
            function add(a, b) return a + b end
            return add(3, 4)
        end
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_blocked_local() {
    valeq!(
        r#"
        do
            local function add(a, b) return a + b end
            return add(3, 4)
        end
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_reverse() {
    valeq!(
        r#"
        add = function(a, b) return a + b end
        return add(3, 4)
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_reverse_local() {
    valeq!(
        r#"
        local add = function(a, b) return a + b end
        return add(3, 4)
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_reverse_blocked() {
    valeq!(
        r#"
        do
            add = function(a, b) return a + b end
            return add(3, 4)
        end
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_reverse_local_blocked() {
    valeq!(
        r#"
        do
            local add = function(a, b) return a + b end
            return add(3, 4)
        end
        "#,
        ExVal::Integer(7)
    );
}

#[test]
fn function_multiple_returns() {
    valeq!(
        r#"
        function get_two_values() return 10, 20 end
        local a, b = get_two_values()
        return a + b
    "#,
        ExVal::Integer(30)
    );
}

#[test]
fn recursive_function() {
    valeq!(
        r#"
        function factorial(n)
            if n <= 1 then return 1 else return n * factorial(n - 1) end
        end
        return factorial(5)
    "#,
        ExVal::Integer(120)
    );
}

#[test]
fn function_as_variable() {
    valeq!(
        r#"
        local function square(x) return x * x end
        local func = square
        return func(5)
    "#,
        ExVal::Integer(25)
    );
}

#[test]
fn nested_function_calls() {
    valeq!(
        r#"
        function double(x) return x * 2 end
        function add_one(x) return x + 1 end
        return double(add_one(5))
    "#,
        ExVal::Integer(12)
    );
}

#[test]
fn call_with_string_argument() {
    valeq!(
        r#"
        function id(s) return s end
        return id "hello"
        "#,
        ExVal::String("hello".to_string())
    );
}
