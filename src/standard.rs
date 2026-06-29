use std::rc::Rc;

use gc_arena::{Gc, Mutation};

use crate::{
    error::SiltError,
    function::{NativeFunctionRaw, WrappedFn},
    prelude::VM,
    userdata::{InnerResult, TestEnt},
    value::Value,
};

/// Build a `Value::NativeFunction` from a multi-return native closure — used to
/// hand back the iterator functions from `pairs`/`ipairs`.
fn make_native_multi<'gc>(
    mc: &Mutation<'gc>,
    f: fn(&mut VM<'gc>, &Mutation<'gc>, &[Value<'gc>]) -> Result<Vec<Value<'gc>>, SiltError>,
) -> Value<'gc> {
    Value::NativeFunction(Gc::new(mc, WrappedFn::new(Rc::new(NativeFunctionRaw::new_multi(f)))))
}

pub fn clock<'lua>(_: &mut VM<'lua>, _: &Mutation<'lua>, _: ()) -> InnerResult<'lua> {
    Ok(Value::Number(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
    ))
}

pub fn print<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let s = args
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\t");
    println!("> {}", s);

    #[cfg(target_arch = "wasm32")]
    crate::jprintln(s.as_str());

    Ok(Value::Nil)
}

pub fn table_insert<'lua>(
    _: &mut VM,
    mc: &Mutation<'lua>,
    args: (Value<'lua>, Value<'lua>, Option<Value<'lua>>),
) -> InnerResult<'lua> {
    match &args.0 {
        Value::Table(t) => {
            let mut table = t.borrow_mut(mc);
            match args.2 {
                Some(v) => {
                    let key = args.1;
                    return table.insert(key,v);
                }
                None => {
                    table.push(args.1);
                }
            }
        }
        _ => return Err(crate::LuaError::VmNonTableOperations(args.0.to_error())),
    }
    Ok(Value::Nil)
}

pub fn table_remove<'lua>(
    _: &mut VM,
    mc: &Mutation<'lua>,
    args: (Value<'lua>, Option<Value<'lua>>),
) ->InnerResult<'lua>{
    
    // let key = Value::Integer(self.counter);
    // let value = self.data.remove(&key);
    // self.counter -= 1;


match &args.0 {
        Value::Table(t) => {
            let mut table = t.borrow_mut(mc);
            match args.1 {
                Some(key) => {
                    return table.insert(key, args.0)
                    // let v1 = args.1;
                    // if let Some(ret) = table.set(v1, v2) {
                    //     return Ok(ret);
                    // }
                }
                None => {
                    // table.push(args.1);
                    table.push(args.0);
                }
            }
        }
        _ => return Err(crate::LuaError::VmNonTableOperations(args.0.to_error())),
    }
    Ok(Value::Nil)
}

pub fn setmetatable<'lua>(
    _: &mut VM,
    mc: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    // println!("we caled setmetatable");
    // let t=args.
    // let metatable = args[1].clone();
    match &args[0] {
        Value::Table(t) => t.borrow_mut(mc).set_metatable(args[1].clone()),
        Value::String(_s) => {}
        _ => {
            println!("cant set metatable on this non table"); // TODO
        }
    }
    // Lua's `setmetatable` returns its first argument so `local t = setmetatable({}, mt)`
    // works as the idiomatic constructor pattern.
    Ok(args[0].clone())
}

pub fn getmetatable<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    Ok(if let Value::Table(t) = args[0] {
        t.borrow().get_metatable()
    } else {
        Value::Nil
    })
}

pub fn select<'lua>(_: &mut VM, _: &Mutation<'lua>, _args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    // Value::Nil
    todo!()
}

pub fn test_ent<'lua>(
    vm: &mut VM<'lua>,
    mc: &Mutation<'lua>,
    _args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let e = TestEnt::new(4., 5., 6.);
    Ok(vm.create_userdata(mc, e))
}

// ============================================================================
// Base library (single-return subset)
// ============================================================================

/// In Lua only `false` and `nil` are falsy.
fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Bool(false))
}

/// Coerce a value to f64 for math, accepting numeric strings (Lua semantics).
fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(i) => Some(*i as f64),
        Value::Number(n) => Some(*n),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

pub fn lua_type<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let v = args.first().cloned().unwrap_or(Value::Nil);
    Ok(Value::String(v.type_name().to_string()))
}

pub fn tostring<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let v = args.first().cloned().unwrap_or(Value::Nil);
    Ok(Value::String(v.coerce_string()))
}

pub fn tonumber<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: (Value<'lua>, Option<Value<'lua>>),
) -> InnerResult<'lua> {
    // tonumber(s, base): parse string `s` in the given integer base
    if let Some(base) = args.1 {
        let radix = base.coerce_int();
        if let Value::String(s) = &args.0 {
            if (2..=36).contains(&radix) {
                return Ok(i64::from_str_radix(s.trim(), radix as u32)
                    .map(Value::Integer)
                    .unwrap_or(Value::Nil));
            }
        }
        return Ok(Value::Nil);
    }
    Ok(match args.0 {
        v @ (Value::Integer(_) | Value::Number(_)) => v,
        Value::String(s) => {
            let t = s.trim();
            if let Ok(i) = t.parse::<i64>() {
                Value::Integer(i)
            } else if let Ok(f) = t.parse::<f64>() {
                Value::Number(f)
            } else if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i64::from_str_radix(hex, 16)
                    .map(Value::Integer)
                    .unwrap_or(Value::Nil)
            } else {
                Value::Nil
            }
        }
        _ => Value::Nil,
    })
}

pub fn assert<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let v = args.first().cloned().unwrap_or(Value::Nil);
    if truthy(&v) {
        // Lua returns all its arguments; with single-return we return the first.
        Ok(v)
    } else {
        let msg = args
            .get(1)
            .map(|m| m.coerce_string())
            .unwrap_or_else(|| "assertion failed!".to_string());
        Err(crate::LuaError::Custom(msg))
    }
}

pub fn error<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let msg = args
        .first()
        .map(|m| m.coerce_string())
        .unwrap_or_else(|| "nil".to_string());
    Err(crate::LuaError::Custom(msg))
}

// ============================================================================
// Iteration: next / pairs / ipairs (multi-return)
// ============================================================================

/// `next(t [,k])` — stateless step over a table; returns the next key/value, or
/// a single `nil` when exhausted.
pub fn lua_next<'lua>(
    _: &mut VM<'lua>,
    _: &Mutation<'lua>,
    args: &[Value<'lua>],
) -> Result<Vec<Value<'lua>>, SiltError> {
    let t = match args.first() {
        Some(Value::Table(t)) => *t,
        _ => return Err(SiltError::Custom("bad argument #1 to 'next' (table expected)".into())),
    };
    let key = args.get(1).cloned().unwrap_or(Value::Nil);
    match t.borrow().next_entry(&key) {
        Some((k, v)) => Ok(vec![k, v]),
        None => Ok(vec![Value::Nil]),
    }
}

/// `pairs(t)` → `(next, t, nil)` — the generic-for iteration triple.
pub fn lua_pairs<'lua>(
    _: &mut VM<'lua>,
    mc: &Mutation<'lua>,
    args: &[Value<'lua>],
) -> Result<Vec<Value<'lua>>, SiltError> {
    let t = match args.first() {
        Some(t @ Value::Table(_)) => t.clone(),
        _ => return Err(SiltError::Custom("bad argument #1 to 'pairs' (table expected)".into())),
    };
    Ok(vec![make_native_multi(mc, lua_next), t, Value::Nil])
}

/// The iterator returned by `ipairs`: `iter(t, i)` → `(i+1, t[i+1])` or `nil`.
fn ipairs_iter<'lua>(
    _: &mut VM<'lua>,
    _: &Mutation<'lua>,
    args: &[Value<'lua>],
) -> Result<Vec<Value<'lua>>, SiltError> {
    let t = match args.first() {
        Some(Value::Table(t)) => *t,
        _ => return Err(SiltError::Custom("bad argument to ipairs iterator".into())),
    };
    let i = args.get(1).map(|v| v.coerce_int()).unwrap_or(0) + 1;
    let v = t.borrow().get_value(&Value::Integer(i));
    if matches!(v, Value::Nil) {
        Ok(vec![Value::Nil])
    } else {
        Ok(vec![Value::Integer(i), v])
    }
}

/// `ipairs(t)` → `(iter, t, 0)` — sequential integer-key iteration from 1.
pub fn lua_ipairs<'lua>(
    _: &mut VM<'lua>,
    mc: &Mutation<'lua>,
    args: &[Value<'lua>],
) -> Result<Vec<Value<'lua>>, SiltError> {
    let t = match args.first() {
        Some(t @ Value::Table(_)) => t.clone(),
        _ => return Err(SiltError::Custom("bad argument #1 to 'ipairs' (table expected)".into())),
    };
    Ok(vec![make_native_multi(mc, ipairs_iter), t, Value::Integer(0)])
}

// ============================================================================
// math library
// ============================================================================

fn math_unary<'lua>(
    args: &[Value<'lua>],
    name: &str,
    f: impl Fn(f64) -> f64,
) -> InnerResult<'lua> {
    match args.first().and_then(to_f64) {
        Some(n) => Ok(Value::Number(f(n))),
        None => Err(crate::LuaError::Custom(format!(
            "bad argument #1 to '{}' (number expected)",
            name
        ))),
    }
}

pub fn math_floor<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    match args.first() {
        Some(Value::Integer(i)) => Ok(Value::Integer(*i)),
        Some(v) => match to_f64(v) {
            Some(n) => Ok(Value::Integer(n.floor() as i64)),
            None => Err(crate::LuaError::Custom("bad argument #1 to 'floor'".into())),
        },
        None => Err(crate::LuaError::Custom("bad argument #1 to 'floor'".into())),
    }
}

pub fn math_ceil<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    match args.first() {
        Some(Value::Integer(i)) => Ok(Value::Integer(*i)),
        Some(v) => match to_f64(v) {
            Some(n) => Ok(Value::Integer(n.ceil() as i64)),
            None => Err(crate::LuaError::Custom("bad argument #1 to 'ceil'".into())),
        },
        None => Err(crate::LuaError::Custom("bad argument #1 to 'ceil'".into())),
    }
}

pub fn math_abs<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    match args.first() {
        Some(Value::Integer(i)) => Ok(Value::Integer(i.abs())),
        Some(v) => match to_f64(v) {
            Some(n) => Ok(Value::Number(n.abs())),
            None => Err(crate::LuaError::Custom("bad argument #1 to 'abs'".into())),
        },
        None => Err(crate::LuaError::Custom("bad argument #1 to 'abs'".into())),
    }
}

pub fn math_sqrt<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_unary(&args, "sqrt", f64::sqrt)
}
pub fn math_sin<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_unary(&args, "sin", f64::sin)
}
pub fn math_cos<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_unary(&args, "cos", f64::cos)
}
pub fn math_tan<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_unary(&args, "tan", f64::tan)
}

/// Shared implementation for min/max; `want_max` selects the direction.
fn math_extreme<'lua>(args: &[Value<'lua>], want_max: bool, name: &str) -> InnerResult<'lua> {
    let mut best: Option<(f64, Value)> = None;
    for v in args {
        let f = to_f64(v).ok_or_else(|| {
            crate::LuaError::Custom(format!("bad argument to '{}' (number expected)", name))
        })?;
        let take = match &best {
            None => true,
            Some((bf, _)) => {
                if want_max {
                    f > *bf
                } else {
                    f < *bf
                }
            }
        };
        if take {
            best = Some((f, v.clone()));
        }
    }
    best.map(|(_, v)| v).ok_or_else(|| {
        crate::LuaError::Custom(format!("bad argument #1 to '{}' (value expected)", name))
    })
}

pub fn math_min<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_extreme(&args, false, "min")
}
pub fn math_max<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    math_extreme(&args, true, "max")
}

// Simple xorshift64 PRNG — avoids pulling in a `rand` dependency. State is
// process-wide (thread-local); `math.randomseed` resets it.
thread_local! {
    static RNG: std::cell::Cell<u64> = const { std::cell::Cell::new(0x2545_F491_4F6C_DD1D) };
}
fn next_rand() -> u64 {
    RNG.with(|r| {
        let mut x = r.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        r.set(x);
        x
    })
}

pub fn math_random<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let r = next_rand();
    Ok(match (args.first(), args.get(1)) {
        // no args -> float in [0, 1)
        (None, _) => Value::Number((r >> 11) as f64 / (1u64 << 53) as f64),
        // random(m) -> integer in [1, m]
        (Some(m), None) => {
            let m = m.coerce_int();
            if m < 1 {
                return Err(crate::LuaError::Custom(
                    "bad argument #1 to 'random' (interval is empty)".into(),
                ));
            }
            Value::Integer(1 + (r % m as u64) as i64)
        }
        // random(m, n) -> integer in [m, n]
        (Some(m), Some(n)) => {
            let (m, n) = (m.coerce_int(), n.coerce_int());
            if m > n {
                return Err(crate::LuaError::Custom(
                    "bad argument #2 to 'random' (interval is empty)".into(),
                ));
            }
            let span = (n - m + 1) as u64;
            Value::Integer(m + (r % span) as i64)
        }
    })
}

pub fn math_randomseed<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let seed = args.first().map(|v| v.coerce_int()).unwrap_or(0) as u64;
    // xorshift must never sit at 0
    let seed = if seed == 0 { 0x2545_F491_4F6C_DD1D } else { seed };
    RNG.with(|r| r.set(seed));
    Ok(Value::Nil)
}

// ============================================================================
// string library
// ============================================================================

pub fn string_len<'lua>(_: &mut VM, _: &Mutation<'lua>, args: Vec<Value<'lua>>) -> InnerResult<'lua> {
    let s = args.first().map(|v| v.coerce_string()).unwrap_or_default();
    Ok(Value::Integer(s.len() as i64))
}

pub fn string_upper<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let s = args.first().map(|v| v.coerce_string()).unwrap_or_default();
    Ok(Value::String(s.to_uppercase()))
}

pub fn string_lower<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let s = args.first().map(|v| v.coerce_string()).unwrap_or_default();
    Ok(Value::String(s.to_lowercase()))
}

pub fn string_reverse<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let s = args.first().map(|v| v.coerce_string()).unwrap_or_default();
    Ok(Value::String(s.chars().rev().collect()))
}

pub fn string_rep<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: (Value<'lua>, Value<'lua>),
) -> InnerResult<'lua> {
    let s = args.0.coerce_string();
    let n = args.1.coerce_int();
    Ok(Value::String(if n > 0 { s.repeat(n as usize) } else { String::new() }))
}

pub fn string_sub<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: (Value<'lua>, Value<'lua>, Option<Value<'lua>>),
) -> InnerResult<'lua> {
    let s = args.0.coerce_string();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    // resolve 1-based, possibly-negative indices (Lua semantics)
    let mut i = args.1.coerce_int();
    let mut j = args.2.map(|v| v.coerce_int()).unwrap_or(-1);
    if i < 0 {
        i = (len + i + 1).max(1);
    } else if i == 0 {
        i = 1;
    }
    if j < 0 {
        j = len + j + 1;
    } else if j > len {
        j = len;
    }
    if i > j {
        return Ok(Value::String(String::new()));
    }
    let out: String = chars[(i - 1) as usize..j as usize].iter().collect();
    Ok(Value::String(out))
}

pub fn string_byte<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: (Value<'lua>, Option<Value<'lua>>),
) -> InnerResult<'lua> {
    // NOTE: Lua's range form `byte(s,i,j)` returns multiple values; with the
    // single-return native ABI we only support a single index for now.
    let s = args.0.coerce_string();
    let bytes = s.as_bytes();
    let len = bytes.len() as i64;
    let i = args.1.map(|v| v.coerce_int()).unwrap_or(1);
    let idx = if i < 0 { len + i + 1 } else { i };
    if idx < 1 || idx > len {
        return Ok(Value::Nil);
    }
    Ok(Value::Integer(bytes[(idx - 1) as usize] as i64))
}

pub fn string_char<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let mut s = String::new();
    for v in &args {
        let c = v.coerce_int();
        if !(0..=255).contains(&c) {
            return Err(crate::LuaError::Custom(
                "bad argument to 'char' (value out of range)".into(),
            ));
        }
        s.push(c as u8 as char);
    }
    Ok(Value::String(s))
}

/// `string.format` — supports `%d %i %u %x %X %o %f %e %g %s %c %q %%` with the
/// `- + space # 0` flags plus width and `.precision`. Pattern-style conversions
/// are out of scope.
pub fn string_format<'lua>(
    _: &mut VM,
    _: &Mutation<'lua>,
    args: Vec<Value<'lua>>,
) -> InnerResult<'lua> {
    let fmt = match args.first() {
        Some(v) => v.coerce_string(),
        None => return Err(crate::LuaError::Custom("bad argument #1 to 'format'".into())),
    };
    let mut out = String::new();
    let mut arg_i = 1usize;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // flags
        let mut left = false;
        let mut zero = false;
        let mut plus = false;
        let mut space = false;
        let mut alt = false;
        while let Some(&fc) = chars.peek() {
            match fc {
                '-' => left = true,
                '0' => zero = true,
                '+' => plus = true,
                ' ' => space = true,
                '#' => alt = true,
                _ => break,
            }
            chars.next();
        }
        // width
        let mut width = 0usize;
        while let Some(&d) = chars.peek() {
            if let Some(dig) = d.to_digit(10) {
                width = width * 10 + dig as usize;
                chars.next();
            } else {
                break;
            }
        }
        // precision
        let mut precision: Option<usize> = None;
        if let Some(&'.') = chars.peek() {
            chars.next();
            let mut p = 0usize;
            while let Some(&d) = chars.peek() {
                if let Some(dig) = d.to_digit(10) {
                    p = p * 10 + dig as usize;
                    chars.next();
                } else {
                    break;
                }
            }
            precision = Some(p);
        }
        let conv = match chars.next() {
            Some(c) => c,
            None => return Err(crate::LuaError::Custom("invalid format string to 'format'".into())),
        };
        if conv == '%' {
            out.push('%');
            continue;
        }
        let arg = args.get(arg_i).cloned().unwrap_or(Value::Nil);
        arg_i += 1;
        let body = match conv {
            'd' | 'i' => {
                let n = arg.coerce_int();
                let mut s = n.unsigned_abs().to_string();
                if let Some(p) = precision {
                    if s.len() < p {
                        s = "0".repeat(p - s.len()) + &s;
                    }
                }
                let sign = if n < 0 {
                    "-"
                } else if plus {
                    "+"
                } else if space {
                    " "
                } else {
                    ""
                };
                pad_number(sign, &s, width, left, zero && precision.is_none())
            }
            'u' => {
                let n = arg.coerce_int() as u64;
                pad_number("", &n.to_string(), width, left, zero)
            }
            'x' | 'X' | 'o' => {
                let n = arg.coerce_int() as u64;
                let mut s = match conv {
                    'x' => format!("{:x}", n),
                    'X' => format!("{:X}", n),
                    _ => format!("{:o}", n),
                };
                if let Some(p) = precision {
                    if s.len() < p {
                        s = "0".repeat(p - s.len()) + &s;
                    }
                }
                let prefix = if alt && n != 0 {
                    match conv {
                        'x' => "0x",
                        'X' => "0X",
                        _ => "0",
                    }
                } else {
                    ""
                };
                pad_number(prefix, &s, width, left, zero && precision.is_none())
            }
            'f' | 'F' | 'e' | 'E' | 'g' | 'G' => {
                let n = to_f64(&arg).unwrap_or(0.0);
                let p = precision.unwrap_or(6);
                let mag = match conv {
                    'f' | 'F' => format!("{:.*}", p, n.abs()),
                    'e' => format!("{:.*e}", p, n.abs()),
                    'E' => format!("{:.*E}", p, n.abs()),
                    // %g: trim trailing zeros, pick shortest reasonable form
                    _ => {
                        let s = format!("{}", n.abs());
                        s
                    }
                };
                let sign = if n.is_sign_negative() {
                    "-"
                } else if plus {
                    "+"
                } else if space {
                    " "
                } else {
                    ""
                };
                pad_number(sign, &mag, width, left, zero)
            }
            'c' => {
                let code = arg.coerce_int();
                let s = char::from_u32(code as u32).map(|c| c.to_string()).unwrap_or_default();
                pad_str(&s, width, left)
            }
            's' => {
                let mut s = arg.coerce_string();
                if let Some(p) = precision {
                    s.truncate(p);
                }
                pad_str(&s, width, left)
            }
            'q' => {
                // quoted, escaped string literal
                let s = arg.coerce_string();
                let mut q = String::from("\"");
                for ch in s.chars() {
                    match ch {
                        '"' => q.push_str("\\\""),
                        '\\' => q.push_str("\\\\"),
                        '\n' => q.push_str("\\n"),
                        '\r' => q.push_str("\\r"),
                        '\0' => q.push_str("\\0"),
                        _ => q.push(ch),
                    }
                }
                q.push('"');
                q
            }
            other => {
                return Err(crate::LuaError::Custom(format!(
                    "invalid conversion '%{}' to 'format'",
                    other
                )))
            }
        };
        out.push_str(&body);
    }
    Ok(Value::String(out))
}

/// Pad a string field to `width`, space-filled, honoring left alignment.
fn pad_str(s: &str, width: usize, left: bool) -> String {
    if s.len() >= width {
        return s.to_string();
    }
    let fill = " ".repeat(width - s.len());
    if left {
        format!("{}{}", s, fill)
    } else {
        format!("{}{}", fill, s)
    }
}

/// Pad a numeric field (sign/prefix + magnitude) to `width`. With `zero` and
/// right alignment the padding goes between the sign and the digits.
fn pad_number(sign: &str, mag: &str, width: usize, left: bool, zero: bool) -> String {
    let total = sign.len() + mag.len();
    if total >= width {
        return format!("{}{}", sign, mag);
    }
    let pad = width - total;
    if left {
        format!("{}{}{}", sign, mag, " ".repeat(pad))
    } else if zero {
        format!("{}{}{}", sign, "0".repeat(pad), mag)
    } else {
        format!("{}{}{}", " ".repeat(pad), sign, mag)
    }
}
