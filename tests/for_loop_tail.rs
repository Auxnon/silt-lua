// The numeric for's scope-end POPS is folded into FORLOOP. These cover what
// can jump to (or sit at) the end of a loop body.

use silt_lua::{Compiler, ExVal, Lua};

fn run(src: &str) -> ExVal {
    let mut lua = Lua::new_with_standard();
    lua.run(None, src, &mut Compiler::new()).unwrap_or_else(|e| panic!("{:?}", e))
}

#[test]
fn body_locals_are_dropped_each_iteration() {
    let src = "local s = 0 for i = 1, 10 do local a = i local b = a * 2 s = s + b end return s";
    assert_eq!(run(src), ExVal::Integer(110));
}

#[test]
fn body_ending_in_an_if_with_locals() {
    // the if's exit jump targets the body end, where FORLOOP now sits
    let src = "
        local s = 0
        for i = 1, 10 do
            local a = i
            if a % 2 == 0 then local b = a s = s + b end
        end
        return s";
    assert_eq!(run(src), ExVal::Integer(30));
}

#[test]
fn body_ending_in_an_if_else() {
    // both branches' jumps end at the body end
    let src = "
        local s = 0
        for i = 1, 10 do
            local a = i
            if a % 2 == 0 then s = s + a else local b = a * 100 s = s - b end
        end
        return s";
    assert_eq!(run(src), ExVal::Integer(30 - 2500));
}

#[test]
fn break_and_nested_loops() {
    let src = "
        local s = 0
        for i = 1, 5 do
            for j = 1, 5 do
                local p = i * j
                if p > 6 then break end
                s = s + p
            end
        end
        return s";
    // i=1: 1+2+3+4+5=15, i=2: 2+4+6=12, i=3: 3+6=9, i=4: 4, i=5: 5
    assert_eq!(run(src), ExVal::Integer(45));
}

#[test]
fn captured_loop_variable_still_closes() {
    // a captured local ends the scope with CLOSE_UPVALUES, which is not folded
    let src = "
        local fs = {}
        for i = 1, 3 do fs[i] = function() return i end end
        return fs[1]() + fs[2]() * 10 + fs[3]() * 100";
    assert_eq!(run(src), ExVal::Integer(321));
}

#[test]
fn empty_body_and_values_after_the_loop() {
    let src = "local before = 7 for i = 1, 3 do end local after = 8 return before * 10 + after";
    assert_eq!(run(src), ExVal::Integer(78));
}

#[test]
fn integer_loop_ends_at_maxinteger() {
    // stepping past the bound used to wrap to mininteger and loop forever
    let src = "
        local n = 0
        for i = math.maxinteger - 2, math.maxinteger do n = n + 1 end
        for i = math.mininteger + 2, math.mininteger, -1 do n = n + 10 end
        return n";
    assert_eq!(run(src), ExVal::Integer(33));
}

#[test]
fn descending_float_and_empty_loops() {
    let src = "
        local s = 0
        for i = 10, 1, -3 do s = s * 10 + i end      -- 10, 7, 4, 1
        for x = 0.5, 1.5, 0.5 do s = s + x end       -- + 3.0
        for i = 1, 0 do s = -1 end
        for i = 0, -1, 1 do s = -1 end
        return s";
    assert_eq!(run(src), ExVal::Number(10741.0 + 3.0));
}
