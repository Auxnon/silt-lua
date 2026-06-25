use silt_lua::{simple, test_int, test_number, valeq, ExVal};

// --- Integer arithmetic (Lua 5.3+ integer subtype) ---
test_int!(add_integers, "return 5 + 3", 8);
test_int!(subtract_integers, "return 10 - 4", 6);
test_int!(multiply_integers, "return 6 * 7", 42);
test_int!(unary_minus, "return -42", -42);
test_int!(zero_result, "return 5 - 5", 0);
test_int!(negative_result, "return 3 - 8", -5);

// --- Float arithmetic ---
test_number!(add_floats, "return 2.5 + 1.5", 4.0);
test_number!(subtract_floats, "return 5.7 - 2.2", 3.5);
test_number!(multiply_floats, "return 3.5 * 2.0", 7.0);
// Division always yields a float in Lua 5.3+, even with integer operands.
test_number!(divide_integers, "return 15 / 3", 5.0);
test_number!(divide_floats, "return 7.5 / 2.5", 3.0);
test_number!(division_by_decimal, "return 1 / 2", 0.5);

// --- Precedence (mixed int produces int; division promotes to float) ---
test_int!(mixed_arithmetic, "return 10 + 5 * 2 - 3", 17);
test_number!(complex_expression, "return 2 + 3 * 4 - 5 / 2 + 1", 12.5);

#[test]
fn chained_operations() {
    valeq!("return 1 + 2 + 3 + 4", ExVal::Integer(10));
    valeq!("return 100 - 20 - 30", ExVal::Integer(50));
    valeq!("return 2 * 3 * 4", ExVal::Integer(24));
}

#[test]
fn operator_precedence() {
    valeq!("return 2 + 3 * 4", ExVal::Integer(14));
    valeq!("return 2 * 3 + 4", ExVal::Integer(10));
}

// =====================================================================================
// BROKEN / UNIMPLEMENTED — these encode the correct expected behavior. Remove #[ignore]
// when the referenced PLAN.md item is fixed.
// =====================================================================================

#[test]
fn parentheses_then_operator() {
    valeq!("return (1 + 2) * 3", ExVal::Integer(9));
    valeq!("return (10 + 5) * 2 - 3", ExVal::Integer(27));
    valeq!("return (1 + 2) + 3", ExVal::Integer(6));
    valeq!("return ((1 + 2) * (3 + 4))", ExVal::Integer(21));
    valeq!("return 2 * (3 + (4 - 1))", ExVal::Integer(12));
}

#[test]
fn modulo() {
    valeq!("return 17 % 5", ExVal::Integer(2));
    valeq!("return 5 % 2", ExVal::Integer(1));
    valeq!("return 5.5 % 2", ExVal::Number(1.5)); // floored modulo
    valeq!("return -5 % 3", ExVal::Integer(1)); // floored: sign of divisor
    valeq!("return 5 % -3", ExVal::Integer(-1));
}

#[test]
fn power() {
    valeq!("return 2 ^ 3", ExVal::Number(8.0));
    valeq!("return 2 ^ 10", ExVal::Number(1024.0));
    // right-associative, and binds tighter than unary minus
    valeq!("return 2 ^ 2 ^ 3", ExVal::Number(256.0));
    valeq!("return -2 ^ 2", ExVal::Number(-4.0));
}

#[test]
fn floor_division() {
    valeq!("return 7 // 2", ExVal::Integer(3));
    valeq!("return 7.0 // 2", ExVal::Number(3.0));
    valeq!("return -7 // 2", ExVal::Integer(-4)); // rounds toward -inf
}

#[test]
fn arithmetic_precedence() {
    valeq!("return 2 + 3 % 4", ExVal::Integer(5)); // % binds tighter than +
    valeq!("return 10 // 3 + 1", ExVal::Integer(4));
    valeq!("return 2 * 3 ^ 2", ExVal::Number(18.0)); // ^ tighter than *
}

#[test]
fn bitwise() {
    valeq!("return 6 & 3", ExVal::Integer(2));
    valeq!("return 4 | 1", ExVal::Integer(5));
    valeq!("return 5 ~ 1", ExVal::Integer(4)); // binary xor
    valeq!("return ~0", ExVal::Integer(-1)); // unary not
    valeq!("return ~5", ExVal::Integer(-6));
    valeq!("return 1 << 4", ExVal::Integer(16));
    valeq!("return 256 >> 2", ExVal::Integer(64));
}

#[test]
fn bitwise_precedence() {
    // shift binds tighter than | ; + binds tighter than & ; xor tighter than |
    valeq!("return 1 << 2 | 1", ExVal::Integer(5));
    valeq!("return 7 & 3 + 1", ExVal::Integer(4));
    valeq!("return 2 | 1 ~ 3", ExVal::Integer(2));
    // comparison is looser than bitwise: (5 & 3) == 1
    valeq!("return 5 & 3 == 1", ExVal::Bool(true));
}
