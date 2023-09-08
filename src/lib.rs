use lua::Lua;
use value::Value;

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

use wasm_bindgen::prelude::*;

fn simple(source: &str) -> Value {
    let mut vm = Lua::new();
    vm.load_standard_library();
    match vm.run(source) {
        Ok(v) => v,
        Err(e) => Value::String(Box::new(e[0].to_string())),
    }

    // let e = compiler.get_errors();
    // if e.len() > 0 {
    //     return Value::String(Box::new(e[0].to_string()));
    // }
    // Value::String(Box::new("Unknown error".to_string()))
}

#[wasm_bindgen]
extern "C" {
    pub fn jprintln(s: &str);
}

#[wasm_bindgen]
pub fn run(source: &str) -> String {
    let mut vm = Lua::new();
    vm.load_standard_library();
    match vm.run(source) {
        Ok(v) => v.to_string(),
        Err(e) => e[0].to_string(),
    }
}
macro_rules! valeq {
    ($source:literal, $val:expr) => {
        assert_eq!(simple($source), $val);
    };
}
macro_rules! vstr {
    ($source:literal) => {
        Value::String(Box::new($source.to_string()))
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        chunk::Chunk,
        code::{self, OpCode},
        function::FunctionObject,
        lua::Lua,
        simple,
        token::Token,
        value::Value,
    };
    use std::{mem::size_of, println, rc::Rc};

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
        println!("size of Tester: {}", size_of::<crate::code::Tester>());
        println!("size of Flag: {}", size_of::<crate::token::Flag>());
        println!("size of token: {}", size_of::<Token>());

        println!("size of token: {}", size_of::<Token>());
        // let s = "12345678".as_bytes().len();
        // println!("size of str: {}", s);
        println!(
            "size of silt_error: {}",
            size_of::<crate::error::SiltError>()
        );
        println!(
            "size of error_types: {}",
            size_of::<crate::error::ErrorTypes>()
        );

        println!("size of value: {}", size_of::<crate::value::Value>());
        println!("size of OpCode: {}", size_of::<crate::code::OpCode>());

        assert!(size_of::<Token>() == 24);
        assert!(size_of::<crate::code::OpCode>() == 3);
    }

    #[test]
    fn speed() {
        let source_in = r#"
    start=clock()
    i=1
    a="a"
    while i < 100_000 do
        i = i +1
        a = a .. "1"
    end
    elapsed=clock()-start
    print "done"
    print ("elapsed: "..elapsed)
    return elapsed
    "#;
        let n = if let crate::value::Value::Number(n) = simple(source_in) {
            println!("{} seconds", n);
            n
        } else {
            999999.
        };
        assert!(n < 2.14)
    }
    #[test]
    fn fibby() {
        let source_in = r#"
        function fib(n)
            if n <= 1 then
            return n
            else
                return fib(n-1) + fib(n-2)
            end
        end
      
        for i = 1, 35 do
            sprint i..":"..fib(i)
        end
    "#;
        println!("{}", simple(source_in));

        // let n = if let crate::value::Value::Number(n) = crate::cli(
        //     source_in,
        //     &mut crate::environment::Environment::new_with_std(),
        // ) {
        //     println!("{} seconds", n);
        //     n
        // } else {
        //     999999.
        // };
        // assert!(n < 3.4)
    }
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

        let n = if let crate::value::Value::Number(n) = simple(source_in) {
            println!("{} seconds", n);
            n
        } else {
            999999.
        };
        assert!(n < 3.4)
    }
    #[test]
    fn chunk() {
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
        let blank = FunctionObject::new(None, false);
        let mut tester = FunctionObject::new(None, false);
        tester.set_chunk(c);
        let mut vm = Lua::new();
        if let Err(e) = vm.execute(Rc::new(tester)) {
            println!("{}", e);
        }
    }
    #[test]
    fn pointer() {
        let x = vec![1, 2, 4];
        let x_ptr = x.as_ptr();

        unsafe {
            for i in 0..x.len() {
                println!("{}", *x_ptr);
                assert_eq!(*x_ptr.add(i), 1 << i);
            }
        }
    }
    #[test]
    fn compliance() {
        valeq!(" 1+2", Value::Integer(3));
        valeq!(" '1'..'2'", vstr!("12"));

        valeq!(
            r#"
            local a= 1+2
            a
            "#,
            Value::Integer(3)
        );
        valeq!(
            r#"
            local a= '1'..'2'
            a
            "#,
            vstr!("12")
        );
        valeq!(
            r#"
            local a= 'a'
            sprint a
            a='b'
            sprint a
            local b='c'
            sprint b
            b=a..b
            sprint b
            b
            "#,
            vstr!("bc")
        );
        // TODO ?????
        valeq!(
            r#"
            local a= b=2
            a
            "#,
            Value::Integer(2)
        );
    }
    #[test]
    fn closures() {
        valeq!(
            r#"
do
    local a = 1
    function f1()
       return a
    end

    local b=f1()
    a = 3
    b=b+ f1()
   return b
end
"#,
            Value::Integer(4)
        );
        valeq!(
            r#"
do
function outer()
    local y=0
    local x = 2
    local function middle()
        local function inner()
            return x
        end
        y = y + 1
        return inner
    end

    y = y + 1
    x=x + y
    return middle
end

a = outer()
b = a()
return b
end

"#,
            Value::Integer(4)
        );
    }
}
