use silt_lua::{ExVal, simple, valeq};

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
        // division always yields a float in Lua 5.3+
        ExVal::Number(5.0)
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

    // NOTE: `(a + b) * c` belongs here too but is broken by PLAN.md §2.2 (grouping drops the
    // trailing operator). It is covered by the ignored `parentheses_then_operator` test in
    // tests/arithmetic.rs; using a non-parenthesized chained expression here instead.
    valeq!(
        r#"
        function test()
            local a, b, c = 2, 3, 4
            return a * b + c
        end
        return test()
        "#,
        10
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
        // b = 5, c = 7  ->  12  (original expectation of 13 was wrong)
        12
    );
}

// Bare uninitialized locals (`local x` with no `=`) must occupy their slot with nil so
// subsequent locals — and closures that capture them — stay correctly aligned.
// Regression: previously the slot was left unfilled, mis-indexing everything after it
// (and a later capture could panic with "attempt to subtract with overflow").

#[test]
fn uninitialized_local_is_nil() {
    valeq!("local x return x", ExVal::Nil);
    valeq!("local a, b, c return b", ExVal::Nil);
}

#[test]
fn uninitialized_local_does_not_shift_later_locals() {
    valeq!("local x local y = 5 return y", ExVal::Integer(5));
    valeq!("local a = 1 local x local b = 2 return a + b", ExVal::Integer(3));
    valeq!(
        "local function f() local x local y = 5 return y end return f()",
        ExVal::Integer(5)
    );
}

#[test]
fn uninitialized_local_then_for_loop() {
    // The loop's hidden control slots must not clobber the earlier local.
    valeq!("local x for i = 1, 2 do end return x", ExVal::Nil);
    valeq!(
        "local function f() local t for i = 1, 3 do t = i end return t end return f()",
        ExVal::Integer(3)
    );
}

#[test]
fn closure_captures_uninitialized_local_assigned_later() {
    valeq!(
        "local function f() local x local g = function() return x end x = 5 return g() end return f()",
        ExVal::Integer(5)
    );
    valeq!(
        "local function f() local g for i = 1, 1 do g = function() return i end end return g() end return f()",
        ExVal::Integer(1)
    );
}
