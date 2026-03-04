use silt_lua::{test_number, valeq, ExVal,simple};

test_number!(add_integers, "return 5 + 3", 8.0);
test_number!(subtract_integers, "return 10 - 4", 6.0);
test_number!(multiply_integers, "return 6 * 7", 42.0);
test_number!(divide_integers, "return 15 / 3", 5.0);
test_number!(modulo_operation, "return 17 % 5", 2.0);
test_number!(power_operation, "return 2 ^ 3", 8.0);
test_number!(unary_minus, "return -42", -42.0);

test_number!(add_floats, "return 2.5 + 1.5", 4.0);
test_number!(subtract_floats, "return 5.7 - 2.2", 3.5);
test_number!(multiply_floats, "return 3.5 * 2.0", 7.0);
test_number!(divide_floats, "return 7.5 / 2.5", 3.0);

test_number!(mixed_arithmetic, "return 10 + 5 * 2 - 3", 17.0);
test_number!(parentheses_precedence, "return (10 + 5) * 2 - 3", 27.0);
test_number!(complex_expression, "return 2 + 3 * 4 - 5 / 2 + 1", 11.5);

test_number!(division_by_decimal, "return 1 / 2", 0.5);
test_number!(zero_result, "return 5 - 5", 0.0);
test_number!(negative_result, "return 3 - 8", -5.0);

#[test]
fn chained_operations() {
    valeq!("return 1 + 2 + 3 + 4", ExVal::Number(10.0));
    valeq!("return 100 - 20 - 30", ExVal::Number(50.0));
    valeq!("return 2 * 3 * 4", ExVal::Number(24.0));
}

#[test]
fn operator_precedence() {
    valeq!("return 2 + 3 * 4", ExVal::Number(14.0));
    valeq!("return 2 * 3 + 4", ExVal::Number(10.0));
    valeq!("return 2 ^ 3 * 4", ExVal::Number(32.0));
    valeq!("return 2 * 3 ^ 2", ExVal::Number(18.0));
}
