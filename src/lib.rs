use compiler::Compiler;
use error::ErrorTuple;
use lua::Lua;
use value::ExVal;

mod chunk;
mod code;
pub mod compiler;
mod error;
mod function;
mod lexer;
pub mod lua;
pub mod prelude;
pub mod standard;
pub mod table;
mod token;
mod userdata;
pub mod value;

#[cfg(feature = "vectors")]
pub mod vec;

use wasm_bindgen::prelude::*;

fn simple(source: &str) -> ExVal {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => v,
        Err(e) => ExVal::String(e[0].to_string()),
    }
    // let v = match compiler.try_compile(source) {
    //     Ok(obj) => match vm.run(obj) {
    //         Ok(v) => v,
    //         Err(e) => ExVal::String(Box::new(e[0].to_string())), //runtime error
    //     },
    //     Err(e) => ExVal::String(Box::new(e[0].to_string())), //compile time error
    // };
    // drop(vm);
    // v

    // let e = compiler.get_errors();
    // if e.len() > 0 {
    //     return Value::String(Box::new(e[0].to_string()));
    // }
    // Value::String(Box::new("Unknown error".to_string()))
}

fn complex(source: &str) -> Result<ExVal, ErrorTuple> {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => Ok(v),
        Err(e) => Err(e.get(0).unwrap().clone()),
    }
    //
    // let mut vm = VM::new();
    // vm.load_standard_library();
    // let mut compiler = Compiler::new();
    // match compiler.try_compile(source) {
    //     Ok(obj) => match vm.run(obj) {
    //         Ok(v) => Ok(v),
    //         Err(e) => Err(e.get(0).unwrap().clone()),
    //     },
    //     Err(e) => Err(e.get(0).unwrap().clone()),
    // }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    pub fn jprintln(s: &str);
}

// #[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn run(source: &str) -> String {
    let mut compiler = Compiler::new();
    let mut lua = Lua::new_with_standard();
    match lua.run(source, &mut compiler) {
        Ok(v) => v.to_string(),
        Err(e) => e[0].to_string(),
    }
}

#[allow(unused_macros)]
macro_rules! valeq {
    ($source:literal, $val:expr) => {
        assert_eq!(simple($source), $val);
    };
}

#[allow(unused_macros)]
macro_rules! fails {
    ($source:literal, $val:expr) => {{
        match complex($source) {
            Ok(_) => panic!("Expected error"),
            Err(e) => assert_eq!(e.code, $val),
        }
    }};
}

#[allow(unused_macros)]
macro_rules! vstr {
    ($source:literal) => {
        ExVal::String($source.to_string())
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        chunk::Chunk,
        code::OpCode,
        complex,
        error::SiltError,
        function::FunctionObject,
        prelude::ValueTypes,
        simple,
        token::Token,
        value::{ExVal, Value},
    };
    use std::{mem::size_of, println};

    #[test]
    fn test_32bits() {
        println!("size of i32: {}", size_of::<i32>());
        println!("size of i64: {}", size_of::<i64>());
        println!("size of f32: {}", size_of::<f32>());
        println!("size of f64: {}", size_of::<f64>());
        println!("size of bool: {}", size_of::<bool>());
        println!("size of char: {}", size_of::<char>());
        println!("size of usize: {}", size_of::<usize>());
        println!("size of u8: {}", size_of::<u8>());
        println!("size of &str: {}", size_of::<&str>());
        println!("size of String: {}", size_of::<String>());
        println!("size of Box<str>: {}", size_of::<Box<str>>());
        println!("size of boxed<Strinv> {}", size_of::<Box<String>>());
        println!("size of Operator: {}", size_of::<crate::token::Operator>());
        // println!("size of Tester: {}", size_of::<crate::code::Tester>());
        println!("size of Flag: {}", size_of::<crate::token::Flag>());
        println!("size of token: {}", size_of::<Token>());
        println!("size of yarn: {}", size_of::<byteyarn::Yarn>());
        // let t: usize = 5;
        // let u: NonZeroUsize = NonZeroUsize::new(1).unwrap();

        println!("size of token: {}", size_of::<Token>());
        println!(
            "size of silt_error: {}",
            size_of::<crate::error::SiltError>()
        );
        println!(
            "size of error_types: {}",
            size_of::<crate::error::ValueTypes>()
        );

        println!("size of value: {}", size_of::<crate::value::Value>());
        println!("size of OpCode: {}", size_of::<crate::code::OpCode>());

        assert!(size_of::<Token>() == 24);
        assert!(size_of::<crate::code::OpCode>() == 4);
    }

    #[test]
    fn speed() {
        let source_in = r#"
    start=clock()
    i=1
    a="a"
    while i < 100000 do
        i = i +1
        a = a .. "1"
    end
    elapsed=clock()-start
    print "done "
    print ("elapsed: "..elapsed)
    return {elapsed,i}
    "#;
        let tuple = if let crate::value::ExVal::Table(t) = simple(source_in) {
            let tt = t;
            (
                if let Some(&ExVal::Number(n)) = tt.getn(1) {
                    n
                } else {
                    999999.
                },
                if let Some(&ExVal::Integer(n)) = tt.getn(2) {
                    println!("{} iterations", n);
                    n
                } else {
                    0
                },
            )
        } else {
            panic!("not a table")
        };

        // assert!(tuple.0 < 2.14);
        assert!(tuple.1 == 100_000);
    }

    // #[test]
    // fn fibby() {
    //     let source_in = r#"
    //     function fib(n)
    //         if n <= 1 then
    //         return n
    //         else
    //             return fib(n-1) + fib(n-2)
    //         end
    //     end

    //     for i = 1, 35 do
    //         sprint i..":"..fib(i)
    //     end
    // "#;
    //     println!("{}", simple(source_in));
    // }

    #[test]
    fn fib() {
        let source_in = r#"
        start=clock() 
        function fib(n)
            if n <= 1 then
                return n
            else
                return fib(n-1) + fib(n-2)
            end
        end
        
        for i = 1, 25 do
            print(i..":"..fib(i))
        end
        elapsed=clock()-start
        return elapsed
        "#;

        let n = if let crate::value::ExVal::Number(n) = simple(source_in) {
            println!("{} seconds", n);
            n
        } else {
            999999.
        };
        println!("{} seconds", n);
        // assert!(n < 3.4)
    }
    #[test]
    fn chunk_validity() {
        let mut c = Chunk::new();
        c.write_value(Value::Number(1.2), (1, 1));
        c.write_value(Value::Number(3.4), (1, 2));
        c.write_code(OpCode::ADD, (1, 3));
        c.write_value(Value::Number(9.2), (1, 4));
        c.write_code(OpCode::DIVIDE, (1, 5));
        c.write_code(OpCode::NEGATE, (1, 1));
        c.write_code(OpCode::RETURN, (1, 3));
        c.print_chunk(None);
        println!("-----------------");
        // let blank = FunctionObject::new(None, false);
        let mut tester = FunctionObject::new(None, false);
        tester.set_chunk(c);
        // let lua = Lua::new();
        // lua.run_chunk
        // match vm.execute(Rc::new(tester)) {
        //     Ok(v) => {
        //         assert_eq!(v, ExVal::Number(-0.5));
        //     }
        //     Err(e) => {
        //         panic!("Test should not fail with error: {}", e)
        //     }
        // }
        panic!(" uh oh we can't eval this chunk!");
    }

    #[test]
    fn compliance1() {
        valeq!("return 1+2", ExVal::Integer(3));
    }

    #[test]
    fn compliance2() {
        valeq!("return '1'..'2'", vstr!("12"));
    }

    #[test]
    fn compliance3() {
        valeq!(
            r#"
            local a= 1+2
            return a
            "#,
            ExVal::Integer(3)
        );
    }

    #[test]
    fn compliance4() {
        valeq!(
            r#"
            local a= '1'..'2'
            return a
            "#,
            vstr!("12")
        );
    }

    #[test]
    fn compliance5() {
        valeq!(
            r#"
            local a= 'a'
            a='b'
            local b='c'
            b=a..b
            return b
            "#,
            vstr!("bc")
        );
    }

    #[test]
    fn compliance() {
        valeq!("return 1+2", ExVal::Integer(3));
        valeq!("return '1'..'2'", vstr!("12"));

        valeq!(
            r#"
            local a= 1+2
            return a
            "#,
            ExVal::Integer(3)
        );
        valeq!(
            r#"
            local a= '1'..'2'
            return a
            "#,
            vstr!("12")
        );
        valeq!(
            r#"
            local a= 'a'
            a='b'
            local b='c'
            b=a..b
            return b
            "#,
            vstr!("bc")
        );
    }

    #[test]
    fn string_infererence() {
        valeq!("return '1'+2", ExVal::Integer(3));
        fails!(
            "return 'a1'+2",
            SiltError::ExpOpValueWithValue(
                ValueTypes::String,
                crate::userdata::MetaMethod::Add,
                ValueTypes::Integer
            )
        );
    }

    #[test]
    fn scope() {
        valeq!(
            r#"
            -- for loop closures should capture the loop variable at that point in time and not the final value of the loop variable
            a = {}
            do
                for i = 1, 3 do
                    local function t()
                        return i
                    end
                    a[i] = t
                end

                return a[1]() + a[2]() + a[3]() -- 1+2+3 = 6
            end
            "#,
            ExVal::Integer(6)
        );
        valeq!(
            r#"
            -- Same but test in global scope
            a = {}
            for i = 1, 3 do
                local function t()
                    return i
                end
                a[i] = t
            end

            return a[1]() + a[2]() + a[3]() -- 1+2+3 = 6
            "#,
            ExVal::Integer(6)
        );
    }

    #[test]
    fn closures() {
        valeq!(
            r#"
            do
                local a = 1
                local b = 2
                local function f1()
                    local c = 3
                    local function f2()
                        local d = 4
                        c = c + d            -- 7, 11
                        return a + b + c + d -- 1+2+7+4= 14, 1+2+11+4 = 18
                    end
                    return f2
                end
                local e = 1000
                local x = f1()
                return x() + x() -- 14+18 = 32
            end
            "#,
            ExVal::Integer(32)
        );
    }

    #[test]
    fn closures2() {
        valeq!(
            r#"
            do
                local a = 1
                function f1()
                    return a
                end

                local b = f1()
                a = 3
                b = b + f1()
                return b --4
            end
            "#,
            ExVal::Integer(4)
        );
        valeq!(
            r#"
            do
                function outer()
                    local y = 0
                    local x = 2
                    local function middle()
                        local function inner()
                            return x
                        end
                        y = y + 1
                        return inner
                    end
            
                    y = y + 1
                    x = x + y
                    return middle
                end
            
                a = outer()
                b = a()
                return b()
            end
            "#,
            ExVal::Integer(3)
        );
    }
    #[test]
    fn call_string() {
        let source_in = r#"
        function call_string(s)
            return s
        end

        return call_string "hello"
        "#;
        assert_eq!(simple(source_in), ExVal::String("hello".to_string()));
    }
}
