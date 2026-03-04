use silt_lua::{valeq,simple,Compiler, ExVal, Lua};

#[test]
fn local_multiple_assignment() {
    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7, 8
            return a
        end
        return test()
        "#,
        5
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7, 8
            return b
        end
        return test()
        "#,
        7
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7, 8
            return c
        end
        return test()
        "#,
        8
    );
}

#[test]
fn local_multiple_sum() {
    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7, 8
            return a + b + c
        end
        return test()
        "#,
        20
    );
}

#[test]
fn local_fewer_values_than_vars() {
    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7
            return a
        end
        return test()
        "#,
        5
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7
            return b
        end
        return test()
        "#,
        7
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5, 7
            return c
        end
        return test()
        "#,
        ExVal::Nil
    );
}

#[test]
fn local_more_values_than_vars() {
    valeq!(
        r#"
        function test()
            local a, b = 5, 7, 8, 9
            return a
        end
        return test()
        "#,
        5
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 7, 8, 9
            return b
        end
        return test()
        "#,
        7
    );
}

#[test]
fn local_no_values() {
    valeq!(
        r#"
        function test()
            local a
            return a
        end
        return test()
        "#,
        ExVal::Nil
    );

    valeq!(
        r#"
        function test()
            local a, b, c
            return a
        end
        return test()
        "#,
        ExVal::Nil
    );

    valeq!(
        r#"
        function test()
            local a, b, c
            return b
        end
        return test()
        "#,
        ExVal::Nil
    );
}

#[test]
fn local_partial_nil() {
    valeq!(
        r#"
        function test()
            local a, b, c = 5
            return a
        end
        return test()
        "#,
        5
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5
            return b
        end
        return test()
        "#,
        ExVal::Nil
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 5
            if b == nil then
                return 999
            end
            return 0
        end
        return test()
        "#,
        999
    );
}

#[test]
fn local_reassignment() {
    valeq!(
        r#"
        function test()
            local a = 5
            a = 10
            return a
        end
        return test()
        "#,
        10
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 7
            a = b
            return a
        end
        return test()
        "#,
        7
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 7
            a, b = b, a
            return a
        end
        return test()
        "#,
        7
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 7
            a, b = b, a
            return b
        end
        return test()
        "#,
        5
    );
}

#[test]
fn local_operations() {
    valeq!(
        r#"
        function test()
            local a, b = 5, 3
            return a + b
        end
        return test()
        "#,
        8
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 3
            return a - b
        end
        return test()
        "#,
        2
    );

    valeq!(
        r#"
        function test()
            local a, b = 5, 3
            return a * b
        end
        return test()
        "#,
        15
    );

    valeq!(
        r#"
        function test()
            local a, b = 10, 2
            return a / b
        end
        return test()
        "#,
        5
    );
}

#[test]
fn local_chained_operations() {
    valeq!(
        r#"
        function test()
            local a, b, c = 2, 3, 4
            return a + b * c
        end
        return test()
        "#,
        14
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 10, 5, 2
            return a - b + c
        end
        return test()
        "#,
        7
    );

    valeq!(
        r#"
        function test()
            local a, b, c = 2, 3, 4
            return (a + b) * c
        end
        return test()
        "#,
        20
    );
}

#[test]
fn local_negative_numbers() {
    valeq!(
        r#"
        function test()
            local a = -5
            return a
        end
        return test()
        "#,
        -5
    );

    valeq!(
        r#"
        function test()
            local a, b = -5, 3
            return a + b
        end
        return test()
        "#,
        -2
    );

    valeq!(
        r#"
        function test()
            local a = 5
            local b = -a
            return b
        end
        return test()
        "#,
        -5
    );
}

#[test]
fn local_nested_scope() {
    valeq!(
        r#"
        function test()
            local a = 5
            do
                local a = 10
                return a
            end
        end
        return test()
        "#,
        10
    );

    valeq!(
        r#"
        function test()
            local a = 5
            do
                local b = 10
            end
            return a
        end
        return test()
        "#,
        5
    );
}

#[test]
fn local_multiple_assignments_chain() {
    valeq!(
        r#"
        function test()
            local a = 1
            local b = 2
            local c = 3
            return a + b + c
        end
        return test()
        "#,
        6
    );

    valeq!(
        r#"
        function test()
            local a = 1
            local b = a + 2
            local c = b + 3
            return c
        end
        return test()
        "#,
        6
    );
}

#[test]
fn local_zero_values() {
    valeq!(
        r#"
        function test()
            local a = 0
            return a
        end
        return test()
        "#,
        0
    );

    valeq!(
        r#"
        function test()
            local a, b = 0, 0
            return a + b
        end
        return test()
        "#,
        0
    );
}

#[test]
fn local_reuse_in_assignment() {
    valeq!(
        r#"
        function test()
            local a = 5
            local b = a
            return b
        end
        return test()
        "#,
        5
    );

    valeq!(
        r#"
        function test()
            local a = 5
            local b = a + 3
            return b
        end
        return test()
        "#,
        8
    );

    valeq!(
        r#"
        function test()
            local a = 5
            local b, c = a, a + 2
            return b + c
        end
        return test()
        "#,
        13
    );
}
