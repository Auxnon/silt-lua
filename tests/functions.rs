use silt_lua::{ExVal,valeq,simple};

#[test]
fn simple_function() {
    valeq!(
        r#"
        function add(a, b)
            return a + b
        end
        return add(3, 4)
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_local() {
    valeq!(
        r#"
        local function multiply(x, y)
            return x * y
        end
        return multiply(6, 7)
    "#,
        ExVal::Number(42.0)
    );
}

#[test]
fn function_no_parameters() {
    let source = r#"
        function get_answer()
            return 42
        end
        return get_answer()
    "#;
    valeq!(source, ExVal::Number(42.0));
}

#[test]
fn function_blocked() {
    valeq!(
        r#"
        do
            function add(a, b)
                return a + b
            end
            return add(3, 4)
        end
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_blocked_local() {
    valeq!(
        r#"
        do
            function add(a, b)
                return a + b
            end
            return add(3, 4)
        end
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_reverse() {
    valeq!(
        r#"
        add= function(a, b)
            return a + b
        end
        return add(3, 4)
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_reverse_local() {
    valeq!(
        r#"
        local add= function(a, b)
            return a + b
        end
        return add(3, 4)
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_reverse_blocked() {
    valeq!(
        r#"
        do
            add=function(a, b)
                return a + b
            end
            return add(3, 4)
        end
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_reverse_local_blocked() {
    valeq!(
        r#"
        do
            local add=function(a, b)
                return a + b
            end
            return add(3, 4)
        end
        "#,
        ExVal::Number(7.0)
    );
}

#[test]
fn function_multiple_returns() {
    let source = r#"
        function get_two_values()
            return 10, 20
        end
        local a, b = get_two_values()
        return a + b
    "#;
    valeq!(source, ExVal::Number(30.0));
}

#[test]
fn recursive_function() {
    let source = r#"
        function factorial(n)
            if n <= 1 then
                return 1
            else
                return n * factorial(n - 1)
            end
        end
        return factorial(5)
    "#;
    valeq!(source, ExVal::Number(120.0));
}

#[test]
fn function_with_closure() {
    let source = r#"
        function make_counter()
            local count = 0
            return function()
                count = count + 1
                return count
            end
        end
        local counter = make_counter()
        counter()
        counter()
        return counter()
    "#;
    valeq!(source, ExVal::Number(3.0));
}

#[test]
fn function_as_variable() {
    let source = r#"
        local function square(x)
            return x * x
        end
        local func = square
        return func(5)
    "#;
    valeq!(source, ExVal::Number(25.0));
}

#[test]
fn nested_function_calls() {
    let source = r#"
        function double(x)
            return x * 2
        end
        function add_one(x)
            return x + 1
        end
        return double(add_one(5))
    "#;
    valeq!(source, ExVal::Number(12.0));
}
