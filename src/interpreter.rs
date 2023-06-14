use crate::{
    environment::Environment,
    error::{ErrorTypes, SiltError},
    expression::Expression,
    statement::Statement,
    token::Operator,
    value::Value,
};

macro_rules! str_op_str{
    ($left:ident $op:tt $right:ident $enu:ident )=>{
        {
            if let Ok(n1) = $left.parse::<i64>() {
                if let Ok(n2) = $right.parse::<i64>() {
                    return Ok(Value::Integer(n1 $op n2));
                }
                if let Ok(n2) = $right.parse::<f64>() {
                    return Ok(Value::Number(int2f!(n1) $op n2));
                }
            }
            if let Ok(n1) = $left.parse::<f64>() {
                if let Ok(n2) = $right.parse::<f64>() {
                    return Ok(Value::Number(n1 $op n2));
                }
            }
            return Err(SiltError::ExpOpValueWithValue(
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::String,
            ));
        }
    }
}

macro_rules! str_op_int{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        {
            if let Ok(n1) = $left.parse::<i64>() {
                    return Ok(Value::Integer(n1 $op $right));

            }
            if let Ok(n1) = $left.parse::<f64>() {
                    return Ok(Value::Number(n1 $op intr2f!($right)));
            }
            return Err(SiltError::ExpOpValueWithValue(
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::Integer,
            ));
        }
    }
}

macro_rules! int_op_str{
    ($left:ident $op:tt $right:ident  $enu:ident)=>{
        {
            if let Ok(n1) = $right.parse::<i64>() {
                    return Ok(Value::Integer($left $op n1));

            }
            if let Ok(n1) = $right.parse::<f64>() {
                    return Ok(Value::Number((intr2f!($left) $op n1)));
            }
            return Err(SiltError::ExpOpValueWithValue(
                ErrorTypes::Integer,
                Operator::$enu,
                ErrorTypes::String,
            ));
        }
    }
}

macro_rules! op_error {
    ($left:ident $op:ident $right:ident ) => {{
        return Err(SiltError::ExpOpValueWithValue($left, Operator::$op, $right));
    }};
}

macro_rules! str_op_num{
    ($left:ident $op:tt $right:ident $enu:ident)=>{
        {
            if let Ok(n1) = $left.parse::<f64>() {
                    return Ok(Value::Number(n1 $op $right));
            }
            return Err(SiltError::ExpOpValueWithValue(
                ErrorTypes::String,
                Operator::$enu,
                ErrorTypes::String,
            ));
        }
    }
}

macro_rules! num_op_str{
    ($left:ident $op:tt $right:ident )=>{
        if let Ok(n1) = $right.parse::<f64>() {
                return Ok(Value::Number(left $op n1));
        }
        return Err(SiltError::ExpAddValueWithValue(
            Value::Number($left),
            Value::String($right),
        ));
    }
}
/** Convert Integer to Float, lossy for now */
macro_rules! int2f {
    ($left:ident) => {
        $left as f64
    };
}

macro_rules! intr2f {
    ($left:ident) => {
        *$left as f64
    };
}

pub fn execute(scope: &mut Environment, statements: &Vec<Statement>) -> Vec<SiltError> {
    // let
    let mut errors: Vec<SiltError> = vec![];
    for s in statements {
        if let Some(e) = match s {
            Statement::Expression(exp) => eval_wrap(scope, exp),
            Statement::Declare { ident, value } => match evaluate(scope, &value) {
                Ok(v) => {
                    scope.set(*ident, v, true);
                    None
                }
                Err(e) => Some(e),
            },
            Statement::If {
                cond,
                then,
                else_cond,
            } => match evaluate(scope, cond) {
                Ok(cond) => {
                    if is_truthy(&cond) {
                        execute(scope, then);
                    } else if let Some(else_cond) = else_cond {
                        execute(scope, else_cond);
                    }
                    None
                }
                Err(e) => Some(e),
            },
            Statement::While { cond, block } => {
                scope.new_scope();
                while let Ok(cond) = evaluate(scope, cond) {
                    if is_truthy(&cond) {
                        execute(scope, &block);
                    } else {
                        break;
                    }
                }
                scope.pop_scope();
                None
            }
            Statement::NumericFor {
                ident,
                start,
                end,
                step,
                block,
            } => {
                // look at this rusty octopus!
                if let Err(e) = (|| {
                    let mut start = evaluate(scope, start)?;
                    let end = evaluate(scope, end)?;

                    let step = match step {
                        Some(s) => evaluate(scope, s)?,
                        None => Value::Integer(1),
                    };
                    scope.new_scope();
                    scope.set(*ident, start.clone(), true);
                    while match eval_binary(&start, &Operator::LessEqual, &end) {
                        Ok(v) => is_truthy(&v),
                        _ => false,
                    } {
                        execute(scope, block);
                        start = eval_binary(&start, &Operator::Add, &step)?;
                        scope.set(*ident, start.clone(), false);
                    }
                    scope.pop_scope();
                    Ok(())
                })() {
                    Some(e)
                } else {
                    None
                }
            }

            Statement::Block(statements) => {
                scope.new_scope();
                // let mut local = Environment::new();
                // local.create_enclosing(scope);
                // execute(&mut local, statements);
                execute(scope, statements);
                scope.pop_scope();
                // drop(local);
                None
            }
            Statement::Print(exp) => print_statement(scope, exp),
            Statement::InvalidStatement => None,
            _ => None,
        } {
            // errors.push(e.clone());
        };
    }

    errors

    // for statement in statements {
    //     match statement {

    //     }
    // }
}

// fn execute_lock(scope: &mut Environment, statements: Vec<Statement>) {
//     let upper = scope;
//     statements.for_each(|s| execute(scope))
// }

fn eval_wrap(global: &mut Environment, exp: &Expression) -> Option<SiltError> {
    //-> Option<SiltError> {
    if let Err(e) = evaluate(global, exp) {
        return Some(e);
    }
    None
}

fn print_statement(global: &mut Environment, exp: &Expression) -> Option<SiltError> {
    match evaluate(global, exp) {
        Ok(v) => println!("> {}", v),
        Err(e) => return Some(e),
    }
    None
}

pub fn evaluate(global: &mut Environment, exp: &Expression) -> Result<Value, SiltError> {
    let v: Value = match exp {
        Expression::Literal { value } => value.clone(),
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let left = evaluate(global, left)?;
            let right = evaluate(global, right)?;
            eval_binary(&left, operator, &right)?
        }
        Expression::Logical {
            left,
            operator,
            right,
        } => {
            let left = evaluate(global, left)?;
            // let right = evaluate(global, *right)?;
            match operator {
                &Operator::Or => {
                    if is_truthy(&left) {
                        left
                    } else {
                        evaluate(global, right)?
                    }
                }
                &Operator::And => {
                    if !is_truthy(&left) {
                        left
                    } else {
                        evaluate(global, right)?
                    }
                }
                _ => return Err(SiltError::InvalidExpressionOperator(operator.clone())), // impossible?
            }
        }
        Expression::Unary { operator, right } => {
            let right = evaluate(global, right)?;
            match operator {
                Operator::Sub => match right {
                    Value::Number(n) => Value::Number(-n),
                    Value::Integer(i) => Value::Integer(-i),
                    v => {
                        return Err(SiltError::ExpInvalidNegation(v.to_error()));
                    }
                },
                Operator::Not => Value::Bool(is_truthy(&right)),
                Operator::Tilde => match right {
                    Value::Integer(i) => Value::Integer(!i),
                    Value::Number(n) => {
                        if n.fract() == 0.0 {
                            Value::Integer(!(n as i64))
                        } else {
                            return Err(SiltError::EvalNoInteger(right.to_error()));
                        }
                    }
                    v => {
                        return Err(SiltError::ExpInvalidBitwise(v.to_error()));
                    }
                },
                _ => return Err(SiltError::InvalidExpressionOperator(operator.clone())),
            }
        }
        Expression::GroupingExpression { expression } => todo!(),
        Expression::Variable { ident } => {
            let v = global.get(&ident);
            match v {
                Value::Number(n) => Value::Number(*n),
                Value::Integer(i) => Value::Integer(*i),
                Value::Bool(b) => Value::Bool(*b),
                Value::String(s) => Value::String(s.clone()),
                Value::Nil => Value::Nil,
                Value::Infinity(f) => Value::Infinity(*f),
                Value::NativeFunction(f) => Value::NativeFunction(*f),
            }
        }
        // Expression::AssignmentExpression { name, value } => todo!(),
        // Expression::EndOfFile => todo!(),
        Expression::InvalidExpression => todo!(),
        Expression::Assign { ident, value } => {
            let val = evaluate(global, value)?;
            global.set(*ident, val, false);
            Value::Nil
        }
        Expression::Call {
            callee,
            args,
            location,
        } => {
            let callee = evaluate(global, callee)?;
            let strict = global.is_strict();

            let mut args = args
                .iter()
                .map(|a| evaluate(global, a))
                .collect::<Result<Vec<Value>, SiltError>>()?;

            if strict {
                // TODO args.len() == function.arity
            }
            match callee {
                // Value::Function(f) => f.call(global, args),
                Value::NativeFunction(f) => f(global, args),
                _ => {
                    return Err(SiltError::NotCallable(callee.to_string()));
                }
            }
        }
    };

    Ok(v)
}

pub fn eval_binary(left: &Value, operator: &Operator, right: &Value) -> Result<Value, SiltError> {
    let val = match (&left, &right) {
        (Value::Number(l), Value::Number(r)) => match operator {
            Operator::Add => Value::Number(l + r),
            Operator::Sub => Value::Number(l - r),
            Operator::Multiply => Value::Number(l * r),
            Operator::Divide => Value::Number(l / r),
            Operator::Modulus => Value::Number(l % r),
            Operator::Equal => Value::Bool(l == r),
            Operator::NotEqual => Value::Bool(l != r),
            Operator::Less => Value::Bool(l < r),
            Operator::LessEqual => Value::Bool(l <= r),
            Operator::Greater => Value::Bool(l > r),
            Operator::GreaterEqual => Value::Bool(l >= r),
            Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator.clone())),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::FloorDivide => Value::Number((l / r).floor()),
            Operator::Exponent => Value::Number(l.powf(*r)),
            Operator::Concat => Value::String((l.to_string() + &r.to_string()).into_boxed_str()),
            Operator::Tilde => todo!(),
            _ => return Err(SiltError::InvalidExpressionOperator(operator.clone())),
        },
        (Value::Integer(l), Value::Integer(r)) => match operator {
            Operator::Add => Value::Integer(l + r),
            Operator::Sub => Value::Integer(l - r),
            Operator::Multiply => Value::Integer(l * r),
            Operator::Divide | Operator::FloorDivide => Value::Integer(l / r),
            Operator::Modulus => Value::Integer(l % r),

            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::Exponent => int_exp(*l, *r),
            Operator::Equal => Value::Bool(l == r),
            Operator::NotEqual => Value::Bool(l != r),
            Operator::Less => Value::Bool(l < r),
            Operator::LessEqual => Value::Bool(l <= r),
            Operator::Greater => Value::Bool(l > r),
            Operator::GreaterEqual => Value::Bool(l >= r),
            Operator::Concat => Value::String((l.to_string() + &r.to_string()).into_boxed_str()),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
            Operator::Tilde => todo!(),
        },
        (Value::Number(l), Value::Integer(r)) => match operator {
            Operator::Add => Value::Number(l + intr2f!(r)),
            Operator::Sub => Value::Number(l - intr2f!(r)),
            Operator::Multiply => Value::Number(l * intr2f!(r)),
            Operator::Divide => Value::Number(l / intr2f!(r)),
            Operator::FloorDivide => Value::Number((l / intr2f!(r)).floor()),
            Operator::Modulus => Value::Number(l % intr2f!(r)),
            Operator::Exponent => Value::Number(l.powf(intr2f!(r))),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::Equal => Value::Bool(*l == intr2f!(r)),
            Operator::NotEqual => Value::Bool(*l != intr2f!(r)),
            Operator::Less => Value::Bool(*l < intr2f!(r)),
            Operator::LessEqual => Value::Bool(*l <= intr2f!(r)),
            Operator::Greater => Value::Bool(*l > intr2f!(r)),
            Operator::GreaterEqual => Value::Bool(*l >= intr2f!(r)),
            Operator::Concat => Value::String((l.to_string() + &r.to_string()).into_boxed_str()),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
            Operator::Tilde => todo!(),
        },
        (Value::Integer(l), Value::Number(r)) => match operator {
            Operator::Add => Value::Number(intr2f!(l) + r),
            Operator::Sub => Value::Number(intr2f!(l) - r),
            Operator::Multiply => Value::Number(intr2f!(l) * r),
            Operator::Divide => Value::Number(intr2f!(l) / r),
            Operator::FloorDivide => Value::Number((intr2f!(l) / r).floor()),
            Operator::Modulus => Value::Number(intr2f!(l) % r),
            Operator::Equal => Value::Bool(intr2f!(l) == *r),
            Operator::NotEqual => Value::Bool(intr2f!(l) != *r),
            Operator::Less => Value::Bool(intr2f!(l) < *r),
            Operator::LessEqual => Value::Bool(intr2f!(l) <= *r),
            Operator::Greater => Value::Bool(intr2f!(l) > *r),
            Operator::GreaterEqual => Value::Bool(intr2f!(l) >= *r),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::Exponent => Value::Number(intr2f!(l).powf(*r)),
            Operator::Concat => Value::String((l.to_string() + &r.to_string()).into_boxed_str()),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
            Operator::Tilde => todo!(),
        },
        (Value::String(l), Value::String(r)) => {
            match operator {
                Operator::Add => {
                    str_op_str!(l + r Add);
                }
                Operator::Sub => {
                    str_op_str!(l - r Sub);
                }
                Operator::Multiply => {
                    str_op_str!(l * r Multiply);
                }
                Operator::Divide => {
                    // always float
                    if let Ok(n1) = l.parse::<f64>() {
                        if let Ok(n2) = r.parse::<f64>() {
                            return Ok(Value::Number(n1 / n2));
                        }
                    }
                    return Err(SiltError::ExpOpValueWithValue(
                        ErrorTypes::String,
                        Operator::Divide,
                        ErrorTypes::String,
                    ));
                }
                Operator::FloorDivide => {
                    if let Ok(n1) = l.parse::<i64>() {
                        if let Ok(n2) = r.parse::<i64>() {
                            return Ok(Value::Integer(n1 / n2));
                        }
                        if let Ok(n2) = r.parse::<f64>() {
                            return Ok(Value::Number((int2f!(n1) / n2).floor()));
                        }
                    }
                    if let Ok(n1) = l.parse::<f64>() {
                        if let Ok(n2) = r.parse::<f64>() {
                            return Ok(Value::Number((n1 / n2).floor()));
                        }
                    }
                    return Err(SiltError::ExpOpValueWithValue(
                        ErrorTypes::String,
                        Operator::FloorDivide,
                        ErrorTypes::String,
                    ));
                }
                Operator::Equal => Value::Bool(l == r),
                Operator::NotEqual => Value::Bool(l != r),
                Operator::Less => Value::Bool(l < r),
                Operator::LessEqual => Value::Bool(l <= r),
                Operator::Greater => Value::Bool(l > r),
                Operator::GreaterEqual => Value::Bool(l >= r),

                // Operator::And => logical_and(left, right),
                // Operator::Or => logical_or(left, right),
                Operator::Modulus => str_op_str!(l % r Modulus),
                Operator::Exponent => {
                    if let Ok(n1) = l.parse::<i64>() {
                        if let Ok(n2) = r.parse::<i64>() {
                            return Ok(int_exp(n1, n2));
                        }
                        if let Ok(n2) = r.parse::<f64>() {
                            return Ok(Value::Number(int2f!(n1).powf(n2)));
                        }
                    }
                    if let Ok(n1) = l.parse::<f64>() {
                        if let Ok(n2) = r.parse::<f64>() {
                            return Ok(Value::Number(n1.powf(n2)));
                        }
                    }
                    return Err(SiltError::ExpOpValueWithValue(
                        ErrorTypes::String,
                        Operator::Exponent,
                        ErrorTypes::String,
                    ));
                }
                Operator::Concat => {
                    if let Value::String(ll) = left {
                        Value::String(((**ll).to_owned() + &**r).into_boxed_str())
                    } else {
                        Value::Nil
                    }
                }
                Operator::Tilde => return Err(SiltError::ExpInvalidBitwise(ErrorTypes::String)),
                Operator::Not | Operator::And | Operator::Or => {
                    return Err(SiltError::InvalidExpressionOperator(operator.clone()))
                }
            }
        }
        (Value::String(l), Value::Number(r)) => match operator {
            Operator::Add => {
                str_op_num!(l + r Add);
            }
            Operator::Sub => {
                str_op_num!(l - r Sub);
            }

            Operator::Multiply => {
                str_op_num!(l * r Multiply);
            }
            Operator::Divide => {
                str_op_num!(l / r Divide);
            }
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::FloorDivide => {
                if let Ok(n1) = l.parse::<f64>() {
                    Value::Number(n1.powf(*r))
                } else {
                    return Err(SiltError::ExpOpValueWithValue(
                        ErrorTypes::String,
                        Operator::FloorDivide,
                        ErrorTypes::Number,
                    ));
                }
            }
            Operator::Modulus => str_op_num!(l % r Modulus),
            Operator::Exponent => {
                if let Ok(n1) = l.parse::<f64>() {
                    return Ok(Value::Number(n1.powf(*r)));
                }
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::String,
                    Operator::Exponent,
                    ErrorTypes::Number,
                ));
            }
            Operator::Concat => {
                if let Value::String(ll) = left {
                    Value::String(((**ll).to_owned() + &r.to_string()).into())
                } else {
                    Value::Nil
                }
            }
            Operator::Equal => Value::Bool(**l == r.to_string()),
            Operator::NotEqual => Value::Bool(**l != r.to_string()),
            op @ (Operator::Less
            | Operator::LessEqual
            | Operator::Greater
            | Operator::GreaterEqual) => {
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::String,
                    op.clone(),
                    ErrorTypes::Number,
                ));
            }
            Operator::Tilde => return Err(SiltError::ExpInvalidBitwise(ErrorTypes::String)),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
        },
        (Value::String(l), Value::Integer(r)) => match operator {
            Operator::Add => str_op_int!(l + r Add),
            Operator::Sub => str_op_int!(l - r Sub),
            Operator::Multiply => str_op_int!(l * r Multiply),
            Operator::Divide => str_op_int!(l / r Divide),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::FloorDivide => {
                if let Ok(n1) = l.parse::<i64>() {
                    return Ok(Value::Integer(n1 / r));
                }
                if let Ok(n1) = l.parse::<f64>() {
                    return Ok(Value::Number((n1 / intr2f!(r)).floor()));
                }
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::String,
                    Operator::FloorDivide,
                    ErrorTypes::Integer,
                ));
            }
            Operator::Modulus => str_op_int!(l % r Modulus),
            Operator::Exponent => {
                if let Ok(n1) = l.parse::<i64>() {
                    return Ok(int_exp(n1, *r));
                }
                if let Ok(n1) = l.parse::<f64>() {
                    return Ok(Value::Number(n1.powf(intr2f!(r))));
                }
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::String,
                    Operator::Exponent,
                    ErrorTypes::Integer,
                ));
            }
            Operator::Concat => {
                if let Value::String(ll) = left {
                    Value::String(((**ll).to_owned() + &r.to_string()).into())
                } else {
                    Value::Nil
                }
            }
            Operator::Equal => Value::Bool(**l == r.to_string()),
            Operator::NotEqual => Value::Bool(**l != r.to_string()),
            op @ (Operator::Less
            | Operator::LessEqual
            | Operator::Greater
            | Operator::GreaterEqual) => {
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::String,
                    op.clone(),
                    ErrorTypes::Integer,
                ));
            }
            Operator::Tilde => return Err(SiltError::ExpInvalidBitwise(ErrorTypes::String)),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
        },
        (Value::Integer(l), Value::String(r)) => match operator {
            Operator::Add => int_op_str!(l + r Add),
            Operator::Sub => int_op_str!(l - r Sub),
            Operator::Multiply => int_op_str!(l * r Multiply),
            Operator::Divide => int_op_str!(l / r Divide),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            Operator::FloorDivide => {
                if let Ok(n1) = r.parse::<i64>() {
                    return Ok(Value::Integer(l / n1));
                }
                if let Ok(n1) = r.parse::<f64>() {
                    return Ok(Value::Number((intr2f!(l) / n1).floor()));
                }
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::Integer,
                    Operator::FloorDivide,
                    ErrorTypes::String,
                ));
            }
            Operator::Modulus => int_op_str!(l % r Modulus),
            Operator::Exponent => {
                if let Ok(n1) = r.parse::<i64>() {
                    return Ok(int_exp(*l, n1));
                }
                if let Ok(n1) = r.parse::<f64>() {
                    return Ok(Value::Number(intr2f!(l).powf(n1)));
                }
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::Integer,
                    Operator::Exponent,
                    ErrorTypes::String,
                ));
            }
            Operator::Concat => Value::String((l.to_string() + &r).into()),
            Operator::Equal => Value::Bool(l.to_string() == **r),
            Operator::NotEqual => Value::Bool(l.to_string() != **r),
            op @ (Operator::Less
            | Operator::LessEqual
            | Operator::Greater
            | Operator::GreaterEqual) => {
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::Integer,
                    op.clone(),
                    ErrorTypes::String,
                ));
            }
            Operator::Tilde => return Err(SiltError::ExpInvalidBitwise(ErrorTypes::String)),
            Operator::Not | Operator::And | Operator::Or => {
                return Err(SiltError::InvalidExpressionOperator(operator.clone()))
            }
        },
        (Value::Integer(_), Value::Bool(_))
        | (Value::Number(_), Value::Bool(_))
        | (Value::Bool(_), Value::Integer(_))
        | (Value::Bool(_), Value::Number(_))
        | (Value::Bool(_), Value::String(_))
        | (Value::String(_), Value::Bool(_))
        | (Value::Bool(_), Value::Nil)
        | (Value::Nil, Value::Bool(_)) => match operator {
            Operator::Equal => Value::Bool(false),
            Operator::NotEqual => Value::Bool(true),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            op => {
                return Err(SiltError::ExpOpValueWithValue(
                    left.to_error(),
                    op.clone(),
                    right.to_error(),
                ))
            }
        },

        (Value::Integer(_) | Value::Number(_) | Value::String(_), Value::Nil) => match operator {
            Operator::Equal => Value::Bool(false),
            Operator::NotEqual => Value::Bool(true),
            // Operator::Or => left,
            // Operator::And => right,
            op => {
                return Err(SiltError::ExpOpValueWithValue(
                    left.to_error(),
                    op.clone(),
                    right.to_error(),
                ))
            }
        },
        (Value::Nil, Value::Integer(_) | Value::Number(_) | Value::String(_)) => match operator {
            Operator::Equal => Value::Bool(false),
            Operator::NotEqual => Value::Bool(true),
            // Operator::Or => right,
            // Operator::And => left,
            op => {
                return Err(SiltError::ExpOpValueWithValue(
                    left.to_error(),
                    op.clone(),
                    right.to_error(),
                ))
            }
        },

        (Value::Number(_), Value::String(_)) => todo!(),
        (Value::Bool(l), Value::Bool(r)) => match operator {
            Operator::Equal => Value::Bool(l == r),
            Operator::NotEqual => Value::Bool(l != r),
            // Operator::And => logical_and(left, right),
            // Operator::Or => logical_or(left, right),
            op => {
                return Err(SiltError::ExpOpValueWithValue(
                    left.to_error(),
                    op.clone(),
                    right.to_error(),
                ))
            }
        },
        (Value::Nil, Value::Nil) => match operator {
            Operator::Equal => Value::Bool(true),
            Operator::NotEqual => Value::Bool(false),
            // Operator::And => Value::Nil,
            // Operator::Or => Value::Nil,
            op => {
                return Err(SiltError::ExpOpValueWithValue(
                    ErrorTypes::Nil,
                    op.clone(),
                    ErrorTypes::Nil,
                ))
            }
        },
        (Value::Integer(_), Value::Infinity(_)) => todo!(),
        (Value::Number(_), Value::Infinity(_)) => todo!(),
        (Value::Bool(_), Value::Infinity(_)) => todo!(),
        (Value::Nil, Value::Infinity(_)) => todo!(),
        (Value::Infinity(_), Value::Integer(_)) => todo!(),
        (Value::Infinity(_), Value::Number(_)) => todo!(),
        (Value::Infinity(_), Value::Bool(_)) => todo!(),
        (Value::Infinity(_), Value::Nil) => todo!(),
        (Value::Infinity(_), Value::Infinity(_)) => todo!(),
        (Value::Infinity(_), Value::String(_)) => todo!(),
        (Value::String(_), Value::Infinity(_)) => todo!(),
        (Value::NativeFunction(_), _) | (_, Value::NativeFunction(_)) => {
            return Err(SiltError::InvalidExpressionOperator(operator.clone()))
        } // _=>Value::Nil,
    };
    Ok(val)
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Nil => false,
        _ => true,
    }
}
/** If 1st truthy return 2nd param, if 1st falsey return 1st param*/
fn logical_and(left: Value, right: Value) -> Value {
    if is_truthy(&left) {
        return right;
    }
    left
}

fn logical_or(left: Value, right: Value) -> Value {
    if is_truthy(&left) {
        return left;
    }
    right
}

fn int_exp(left: i64, right: i64) -> Value {
    if right < 0 {
        Value::Number(int2f!(left).powf(int2f!(right)))
    } else {
        match left.checked_pow(right as u32) {
            Some(n) => Value::Integer(n),
            None => Value::Integer(core::i64::MAX),
        }
    }
}

fn coerce_num(s: &str) -> Result<Value, SiltError> {
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Value::Integer(n));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Value::Number(n));
    }
    Err(SiltError::InvalidNumber(s.to_owned()))
}
