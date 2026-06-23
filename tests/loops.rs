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
#[ignore = "PLAN.md §2.12 — descending numeric for (negative step) never executes the body"]
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
#[ignore = "PLAN.md §2.6 — `break` is not handled by the compiler; loop var is corrupted"]
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
#[ignore = "PLAN.md §2.6 — `break` inside while is not handled"]
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
#[ignore = "PLAN.md §2.6 — repeat/until is not handled by the compiler"]
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
#[ignore = "PLAN.md §2.7/§3 — generic for + ipairs not implemented"]
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
