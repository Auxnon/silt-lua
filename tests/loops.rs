use silt_lua::{simple, valeq, ExVal};

#[test]
fn while_loop() {
    valeq!(
        r#"
        local i = 0
        local sum = 0
        while i < 5 do
            sum = sum + i
            i = i + 1
        end
        return sum
    "#,
        ExVal::Integer(10)
    );
}

#[test]
fn while_loop_with_condition() {
    valeq!(
        r#"
        local x = 1
        local result = 0
        while x <= 4 do
            result = result + x * x
            x = x + 1
        end
        return result
    "#,
        ExVal::Integer(30)
    );
}

#[test]
fn numeric_for_loop() {
    valeq!(
        r#"
        local sum = 0
        for i = 1, 5 do
            sum = sum + i
        end
        return sum
    "#,
        ExVal::Integer(15)
    );
}

#[test]
fn numeric_for_loop_with_step() {
    valeq!(
        r#"
        local sum = 0
        for i = 2, 10, 2 do
            sum = sum + i
        end
        return sum
    "#,
        ExVal::Integer(30)
    );
}

#[test]
fn numeric_for_descending() {
    valeq!(
        r#"
        local sum = 0
        for i = 5, 1, -1 do
            sum = sum + i
        end
        return sum
    "#,
        ExVal::Integer(15)
    );
    // multi-step descending and an empty descending range
    valeq!("local s=0 for i=10,2,-2 do s=s+i end return s", ExVal::Integer(30));
    valeq!("local n=0 for i=5,10,-1 do n=n+1 end return n", ExVal::Integer(0));
}

#[test]
fn nested_loops() {
    valeq!(
        r#"
        local sum = 0
        for i = 1, 3 do
            for j = 1, 2 do
                sum = sum + i * j
            end
        end
        return sum
    "#,
        ExVal::Integer(18)
    );
}

// =====================================================================================
// BROKEN — see PLAN.md
// =====================================================================================

#[test]
fn loop_with_break() {
    valeq!(
        r#"
        local sum = 0
        for i = 1, 10 do
            if i > 5 then break end
            sum = sum + i
        end
        return sum
    "#,
        ExVal::Integer(15)
    );
}

#[test]
fn while_with_break() {
    valeq!(
        r#"
        local i = 0
        while true do
            i = i + 1
            if i >= 5 then break end
        end
        return i
    "#,
        ExVal::Integer(5)
    );
}

#[test]
fn repeat_until_loop() {
    valeq!(
        r#"
        local i = 0
        local sum = 0
        repeat
            sum = sum + i
            i = i + 1
        until i >= 5
        return sum
    "#,
        ExVal::Integer(10)
    );
}

#[test]
fn nested_break_only_exits_inner() {
    valeq!(
        r#"
        local count = 0
        for i = 1, 3 do
            for j = 1, 3 do
                if j == 2 then break end
                count = count + 1
            end
        end
        return count
    "#,
        ExVal::Integer(3)
    );
}

#[test]
fn repeat_until_with_break() {
    valeq!(
        "local i = 0 repeat i = i + 1 if i == 3 then break end until i >= 10 return i",
        ExVal::Integer(3)
    );
}

#[test]
fn repeat_until_sees_body_local() {
    // the `until` condition can reference a local declared in the body
    valeq!(
        "local i = 0 repeat local done = i >= 4 i = i + 1 until done return i",
        ExVal::Integer(5)
    );
}

#[test]
fn while_body_local_is_scoped() {
    // regression: a local declared in a while body must be popped each iteration
    valeq!(
        "local s = 0 local i = 1 while i <= 3 do local x = i * 2 s = s + x i = i + 1 end return s",
        ExVal::Integer(12)
    );
}

#[test]
fn numeric_for_non_literal_start() {
    // Regression: a non-literal numeric-for start (a variable or `#a`) went through
    // `named_variable`, whose multi-assign detection ate the loop's separator comma,
    // yielding "Expected ','". Start/limit/step now disable multivar detection.
    valeq!(
        "local a = {1, 2, 3} local s = 0 for i = #a, 5 do s = s + i end return s",
        ExVal::Integer(12) // 3 + 4 + 5
    );
    valeq!(
        "local n = 2 local s = 0 for i = n, 4 do s = s + i end return s",
        ExVal::Integer(9) // 2 + 3 + 4
    );
    // `#a` in the limit AND a step (comma-separated) also parse.
    valeq!(
        "local a = {1, 2, 3} local s = 0 for i = 1, #a, 1 do s = s + i end return s",
        ExVal::Integer(6)
    );
    // `#a` start with a descending step.
    valeq!(
        "local a = {1, 2, 3, 4} local s = 0 for i = #a, 1, -1 do s = s + i end return s",
        ExVal::Integer(10)
    );
}

#[test]
fn generic_for_ipairs() {
    valeq!(
        r#"
        local t = {10, 20, 30}
        local sum = 0
        for i, v in ipairs(t) do
            sum = sum + v
        end
        return sum
    "#,
        ExVal::Integer(60)
    );
}
