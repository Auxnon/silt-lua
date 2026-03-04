use silt_lua::{ExVal,valeq,simple};


#[test]
fn while_loop() {
    let source = r#"
        local i = 0
        local sum = 0
        while i < 5 do
            sum = sum + i
            i = i + 1
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(10.0));
}

#[test]
fn repeat_until_loop() {
    let source = r#"
        local i = 0
        local sum = 0
        repeat
            sum = sum + i
            i = i + 1
        until i >= 5
        return sum
    "#;
    valeq!(source, ExVal::Number(10.0));
}

#[test]
fn numeric_for_loop() {
    let source = r#"
        local sum = 0
        for i = 1, 5 do
            sum = sum + i
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(15.0));
}

#[test]
fn numeric_for_loop_with_step() {
    let source = r#"
        local sum = 0
        for i = 2, 10, 2 do
            sum = sum + i
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(30.0));
}

#[test]
fn nested_loops() {
    let source = r#"
        local sum = 0
        for i = 1, 3 do
            for j = 1, 2 do
                sum = sum + i * j
            end
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(18.0));
}

#[test]
fn loop_with_break() {
    let source = r#"
        local sum = 0
        for i = 1, 10 do
            if i > 5 then
                break
            end
            sum = sum + i
        end
        return sum
    "#;
    valeq!(source, ExVal::Number(15.0));
}

#[test]
fn while_loop_with_condition() {
    let source = r#"
        local x = 1
        local result = 0
        while x <= 4 do
            result = result + x * x
            x = x + 1
        end
        return result
    "#;
    valeq!(source, ExVal::Number(30.0));
}
