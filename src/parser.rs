pub mod parser {
    use crate::{
        error::{ErrorTuple, ErrorTypes, Location, SiltError},
        expression::Expression,
        token::{Operator, Token},
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

    // macro_rules! val_err {
    //     ($left:ident,$right:ident) => {
    //         return Err(SiltError::ExpAddValueWithValue(
    //             Value::String($left),
    //             Value::String($right),
    //         ));
    //     };
    // }
    pub struct Parser {
        pub iterator: std::iter::Peekable<std::vec::IntoIter<Token>>,
        pub locations: Vec<Location>,
        pub current: usize,
        pub errors: Vec<ErrorTuple>,
        pub valid: bool,
    }
    impl Parser {
        pub fn new(t: Vec<Token>, p: Vec<Location>) -> Parser {
            let ee = t.into_iter().peekable();
            // let tt = t.iter().peekable();
            Parser {
                iterator: ee,
                locations: p,
                current: 0,
                errors: vec![],
                valid: true,
            }
        }
        // pub fn advance(&mut self) -> Option<Token> {
        //     self.current += 1;
        //     self.iterator.next().cloned()
        // }
        fn error(&mut self, code: SiltError) {
            self.valid = false;
            self.errors.push(ErrorTuple {
                code,
                location: self.locations[self.current],
            });
        }
        fn syncronize(&mut self) {
            while match self.peek() {
            Some(&Token::Class)
            | Some(&Token::Function)
            | Some(&Token::Do)
            | Some(&Token::For)
            | Some(&Token::If)
            | Some(&Token::While)
            // | Some(&Token::Print)
            | Some(&Token::Return) => false,
            _ => true,
        } {
                self.eat();
            }
        }

        // _ => Value::Nil,

        pub fn evaluate(&mut self, exp: Expression) -> Result<Value, SiltError> {
            let v = match exp {
                Expression::Literal { value } => value,
                Expression::BinaryExpression {
                    left,
                    operator,
                    right,
                } => {
                    let left = self.evaluate(*left)?;
                    let right = self.evaluate(*right)?;
                    Self::eval_binary(left, operator, right)?
                }
                Expression::Unary { operator, right } => {
                    let right = self.evaluate(*right)?;
                    match operator {
                        Operator::Sub => match right {
                            Value::Number(n) => Value::Number(-n),
                            Value::Integer(i) => Value::Integer(-i),
                            v => {
                                return Err(SiltError::ExpInvalidNegation(v));
                            }
                        },
                        Operator::Not => Value::Bool(Self::is_truthy(&right)),
                        _ => return Err(SiltError::InvalidExpressionOperator(operator)),
                    }
                }
                Expression::GroupingExpression { expression } => todo!(),
                Expression::VariableExpression { name } => todo!(),
                Expression::AssignmentExpression { name, value } => todo!(),
                Expression::EndOfFile => todo!(),
                Expression::InvalidExpression => todo!(),
            };

            Ok(v)
        }

        pub fn eval_binary(
            left: Value,
            operator: Operator,
            right: Value,
        ) -> Result<Value, SiltError> {
            let val = match (&left, &right) {
                (Value::Number(l), Value::Number(r)) => match operator {
                    Operator::Add => Value::Number(l + r),
                    Operator::Sub => Value::Number(l - r),
                    Operator::Multiply => Value::Number(l * r),
                    Operator::Divide => Value::Number(l / r),
                    Operator::Modulus => Value::Number(l % r),
                    Operator::Equal => Value::Bool(l == r),
                    Operator::NotEqual => Value::Bool(l != r),
                    Operator::LessThan => Value::Bool(l < r),
                    Operator::LessThanOrEqual => Value::Bool(l <= r),
                    Operator::GreaterThan => Value::Bool(l > r),
                    Operator::GreaterThanOrEqual => Value::Bool(l >= r),
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    Operator::FloorDivide => Value::Number((l / r).floor()),
                    Operator::Exponent => Value::Number(l.powf(*r)),
                    Operator::Concat => Value::String(l.to_string() + &r.to_string()),
                },
                (Value::Integer(l), Value::Integer(r)) => match operator {
                    Operator::Add => Value::Integer(l + r),
                    Operator::Sub => Value::Integer(l - r),
                    Operator::Multiply => Value::Integer(l * r),
                    Operator::Divide | Operator::FloorDivide => Value::Integer(l / r),
                    Operator::Modulus => Value::Integer(l % r),

                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    Operator::Exponent => Self::int_exp(*l, *r),
                    Operator::Equal => Value::Bool(l == r),
                    Operator::NotEqual => Value::Bool(l != r),
                    Operator::LessThan => Value::Bool(l < r),
                    Operator::LessThanOrEqual => Value::Bool(l <= r),
                    Operator::GreaterThan => Value::Bool(l > r),
                    Operator::GreaterThanOrEqual => Value::Bool(l >= r),
                    Operator::Concat => Value::String(l.to_string() + &r.to_string()),
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
                },
                (Value::Number(l), Value::Integer(r)) => match operator {
                    Operator::Add => Value::Number(l + intr2f!(r)),
                    Operator::Sub => Value::Number(l - intr2f!(r)),
                    Operator::Multiply => Value::Number(l * intr2f!(r)),
                    Operator::Divide => Value::Number(l / intr2f!(r)),
                    Operator::FloorDivide => Value::Number((l / intr2f!(r)).floor()),
                    Operator::Modulus => Value::Number(l % intr2f!(r)),
                    Operator::Exponent => Value::Number(l.powf(intr2f!(r))),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    Operator::Equal => Value::Bool(*l == intr2f!(r)),
                    Operator::NotEqual => Value::Bool(*l != intr2f!(r)),
                    Operator::LessThan => Value::Bool(*l < intr2f!(r)),
                    Operator::LessThanOrEqual => Value::Bool(*l <= intr2f!(r)),
                    Operator::GreaterThan => Value::Bool(*l > intr2f!(r)),
                    Operator::GreaterThanOrEqual => Value::Bool(*l >= intr2f!(r)),
                    Operator::Concat => Value::String(l.to_string() + &r.to_string()),
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
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
                    Operator::LessThan => Value::Bool(intr2f!(l) < *r),
                    Operator::LessThanOrEqual => Value::Bool(intr2f!(l) <= *r),
                    Operator::GreaterThan => Value::Bool(intr2f!(l) > *r),
                    Operator::GreaterThanOrEqual => Value::Bool(intr2f!(l) >= *r),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    Operator::Exponent => Value::Number(intr2f!(l).powf(*r)),
                    Operator::Concat => Value::String(l.to_string() + &r.to_string()),
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
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
                        Operator::LessThan => Value::Bool(l < r),
                        Operator::LessThanOrEqual => Value::Bool(l <= r),
                        Operator::GreaterThan => Value::Bool(l > r),
                        Operator::GreaterThanOrEqual => Value::Bool(l >= r),

                        Operator::And => Self::logical_and(left, right),
                        Operator::Or => Self::logical_or(left, right),
                        Operator::Modulus => str_op_str!(l % r Modulus),
                        Operator::Exponent => {
                            if let Ok(n1) = l.parse::<i64>() {
                                if let Ok(n2) = r.parse::<i64>() {
                                    return Ok(Self::int_exp(n1, n2));
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
                                Value::String(ll + r)
                            } else {
                                Value::Nil
                            }
                        }
                        Operator::Not => {
                            return Err(SiltError::InvalidExpressionOperator(operator))
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
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
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
                            Value::String(ll + &r.to_string())
                        } else {
                            Value::Nil
                        }
                    }
                    Operator::Equal => Value::Bool(*l == r.to_string()),
                    Operator::NotEqual => Value::Bool(*l != r.to_string()),
                    op @ (Operator::LessThan
                    | Operator::LessThanOrEqual
                    | Operator::GreaterThan
                    | Operator::GreaterThanOrEqual) => {
                        return Err(SiltError::ExpOpValueWithValue(
                            ErrorTypes::String,
                            op,
                            ErrorTypes::Number,
                        ));
                    }
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
                },
                (Value::String(l), Value::Integer(r)) => match operator {
                    Operator::Add => str_op_int!(l + r Add),
                    Operator::Sub => str_op_int!(l - r Sub),
                    Operator::Multiply => str_op_int!(l * r Multiply),
                    Operator::Divide => str_op_int!(l / r Divide),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
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
                            return Ok(Self::int_exp(n1, *r));
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
                            Value::String(ll + &r.to_string())
                        } else {
                            Value::Nil
                        }
                    }
                    Operator::Equal => Value::Bool(*l == r.to_string()),
                    Operator::NotEqual => Value::Bool(*l != r.to_string()),
                    op @ (Operator::LessThan
                    | Operator::LessThanOrEqual
                    | Operator::GreaterThan
                    | Operator::GreaterThanOrEqual) => {
                        return Err(SiltError::ExpOpValueWithValue(
                            ErrorTypes::String,
                            op,
                            ErrorTypes::Integer,
                        ));
                    }
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
                },
                (Value::Integer(l), Value::String(r)) => match operator {
                    Operator::Add => int_op_str!(l + r Add),
                    Operator::Sub => int_op_str!(l - r Sub),
                    Operator::Multiply => int_op_str!(l * r Multiply),
                    Operator::Divide => int_op_str!(l / r Divide),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
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
                            return Ok(Self::int_exp(*l, n1));
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
                    Operator::Concat => Value::String(l.to_string() + &r),
                    Operator::Equal => Value::Bool(l.to_string() == *r),
                    Operator::NotEqual => Value::Bool(l.to_string() != *r),
                    op @ (Operator::LessThan
                    | Operator::LessThanOrEqual
                    | Operator::GreaterThan
                    | Operator::GreaterThanOrEqual) => {
                        return Err(SiltError::ExpOpValueWithValue(
                            ErrorTypes::Integer,
                            op,
                            ErrorTypes::String,
                        ));
                    }
                    Operator::Not => return Err(SiltError::InvalidExpressionOperator(operator)),
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
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    op => {
                        return Err(SiltError::ExpOpValueWithValue(
                            left.to_error(),
                            op,
                            right.to_error(),
                        ))
                    }
                },

                (Value::Integer(_) | Value::Number(_) | Value::String(_), Value::Nil) => {
                    match operator {
                        Operator::Equal => Value::Bool(false),
                        Operator::NotEqual => Value::Bool(true),
                        Operator::Or => left,
                        Operator::And => right,
                        op => {
                            return Err(SiltError::ExpOpValueWithValue(
                                left.to_error(),
                                op,
                                right.to_error(),
                            ))
                        }
                    }
                }
                (Value::Nil, Value::Integer(_) | Value::Number(_) | Value::String(_)) => {
                    match operator {
                        Operator::Equal => Value::Bool(false),
                        Operator::NotEqual => Value::Bool(true),
                        Operator::Or => right,
                        Operator::And => left,
                        op => {
                            return Err(SiltError::ExpOpValueWithValue(
                                left.to_error(),
                                op,
                                right.to_error(),
                            ))
                        }
                    }
                }

                (Value::Number(_), Value::String(_)) => todo!(),
                (Value::Bool(l), Value::Bool(r)) => match operator {
                    Operator::Equal => Value::Bool(l == r),
                    Operator::NotEqual => Value::Bool(l != r),
                    Operator::And => Self::logical_and(left, right),
                    Operator::Or => Self::logical_or(left, right),
                    op => {
                        return Err(SiltError::ExpOpValueWithValue(
                            left.to_error(),
                            op,
                            right.to_error(),
                        ))
                    }
                },
                (Value::Nil, Value::Nil) => match operator {
                    Operator::Equal => Value::Bool(true),
                    Operator::NotEqual => Value::Bool(false),
                    Operator::And => Value::Nil,
                    Operator::Or => Value::Nil,
                    op => {
                        return Err(SiltError::ExpOpValueWithValue(
                            ErrorTypes::Nil,
                            op,
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
                // _=>Value::Nil,
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
            if Self::is_truthy(&left) {
                return right;
            }
            left
        }

        fn logical_or(left: Value, right: Value) -> Value {
            if Self::is_truthy(&left) {
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
        fn coerce_num(&self, s: &str) -> Result<Value, SiltError> {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Value::Integer(n));
            }
            if let Ok(n) = s.parse::<f64>() {
                return Ok(Value::Number(n));
            }
            Err(SiltError::InvalidNumber(s.to_owned()))
        }
        // fn next_statement(&self) -> bool {}

        pub fn eat(&mut self) {
            self.current += 1;
            println!(
                "{}",
                match self.iterator.next() {
                    Some(t) => format!("{}", t),
                    None => format!("None"),
                }
            );
        }
        pub fn next(&mut self) -> Option<Token> {
            self.current += 1;
            self.iterator.next()
        }
        /** only use after peek */
        pub fn eat_out(&mut self) -> Token {
            self.current += 1;
            self.iterator.next().unwrap()
        }
        fn peek(&mut self) -> Option<&Token> {
            self.iterator.peek()
        }
        pub fn parse(&mut self) -> Expression {
            self.expression()
        }
        // op unary or primary
        pub fn unary(&mut self) -> Expression {
            if let Some(&Token::Op((Operator::Sub | Operator::Not))) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.unary();
                Expression::Unary {
                    operator,
                    right: Box::new(right),
                }
            } else {
                match self.primary() {
                    Ok(e) => e,
                    Err(code) => {
                        self.error(code);
                        Expression::InvalidExpression
                    }
                }
            }
        }
        // pub fn program(&mut self) -> Vec<Statement> {
        //     let mut statements = vec![];
        //     while let Some(_) = self.peek() {
        //         statements.push(self.declaration());
        //     }
        //     statements
        // }
        // pub fn statement(&mut self) -> Statement {
        //     if let Some(&Token::Print) = self.peek() {
        //         self.eat();
        //         let value = self.expression();
        //         if let Token::Semicolon = self.eat_out() {
        //         } else {
        //             self.error(SiltError::MissingSemicolon);
        //         }
        //         Statement::PrintStatement { value }
        //     } else {
        //         self.expression_statement()
        //     }
        // }
        // pub fn expression_statement(&mut self) -> Statement {
        //     let value = self.expression();
        //     if let Token::Semicolon = self.eat_out() {
        //     } else {
        //         self.error(SiltError::MissingSemicolon);
        //     }
        //     Statement::ExpressionStatement { value }
        // }

        pub fn expression(&mut self) -> Expression {
            self.equality()
        }
        pub fn equality(&mut self) -> Expression {
            let mut exp = self.comparison();
            while let Some(&Token::Op((Operator::NotEqual | Operator::Equal))) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.comparison();
                exp = Expression::BinaryExpression {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }
        fn comparison(&mut self) -> Expression {
            let mut exp = self.term();

            while let Some(&Token::Op(
                (Operator::LessThan
                | Operator::LessThanOrEqual
                | Operator::GreaterThan
                | Operator::GreaterThanOrEqual),
            )) = self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let right = self.term();
                exp = Expression::BinaryExpression {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }
        fn term(&mut self) -> Expression {
            let mut exp = self.factor();
            while let Some(&Token::Op((Operator::Add | Operator::Sub))) = self.peek() {
                let operator = Self::de_op(self.eat_out());
                let right = self.factor();
                exp = Expression::BinaryExpression {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }
        fn factor(&mut self) -> Expression {
            let mut exp = self.unary();
            while let Some(&Token::Op(Operator::Multiply | Operator::Divide | Operator::Modulus)) =
                self.peek()
            {
                let operator = Self::de_op(self.eat_out());
                let right = self.unary();
                exp = Expression::BinaryExpression {
                    left: Box::new(exp),
                    operator,
                    right: Box::new(right),
                };
            }
            exp
        }
        fn primary(&mut self) -> Result<Expression, SiltError> {
            // let err
            let t = self.next();
            match t {
                Some(Token::Number(n)) => Ok(Expression::Literal {
                    value: Value::Number(n),
                }),
                Some(Token::True) => Ok(Expression::Literal {
                    value: Value::Bool(true),
                }),
                Some(Token::False) => Ok(Expression::Literal {
                    value: Value::Bool(false),
                }),
                Some(Token::Nil) => Ok(Expression::Literal { value: Value::Nil }),

                Some(Token::OpenParen) => {
                    let exp = self.expression();
                    if let Token::CloseParen = self.eat_out() {
                    } else {
                        self.error(SiltError::UnterminatedParenthesis);
                    }

                    Ok(Expression::GroupingExpression {
                        expression: Box::new(exp),
                    })
                }
                Some(Token::Identifier(i)) => Ok(Expression::VariableExpression { name: i }),
                Some(Token::EOF) => Ok(Expression::EndOfFile),
                Some(Token::Op(o)) => Err(SiltError::InvalidExpressionOperator(o)),
                _ => Err(SiltError::EarlyEndOfFile), // Expression::EndOfFile
            }
        }
        fn de_op(t: Token) -> Operator {
            if let Token::Op(o) = t {
                return o;
            }
            panic!("Bad operator") // can this happen
        }
    }
}
