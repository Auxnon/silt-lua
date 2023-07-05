use core::panic;
use std::vec;

use crate::{chunk::Chunk, compiler::compiler::Compiler, lexer::TokenResult, vm::VM};

mod chunk;
mod code;
mod compiler;
mod environment;
mod error;
mod expression;
mod function;
mod interpreter;
mod lexer;
mod parser;
mod resolver;
mod standard;
mod statement;
mod token;
mod userdata;
mod value;
mod vm;
fn main() {
    // println!("Hello, world!");
    // let source = r#"
    // abc
    // abc2
    // bc=5--hi there
    // a=1
    // b=2
    // c=a+b
    // d='hello'
    // e="world"
    // f=[[multi
    // line]]
    // print(c)
    // if a==1 then
    // a=2
    // end
    // print(a)
    // let source_in =
    // "#;
    // let source_in = r#"
    // a=1
    // while a < 10_000_00 do
    //     a = a + 1
    // end
    // print a
    // "#;

    //benchmark
    // let source_in = r#"
    // start=clock()
    // i=1
    // a="a"
    // while i < 100_000 do
    //     i = i +1
    //     a = a .. "1"
    // end
    // elapsed=clock()-start
    // print "done"
    // print elapsed
    // elapsed
    // "#;

    // func test
    // let source_in = r#"

    // n=1
    // function b(h) print 'b'..n n=h print 'c'..n end

    // function a() print 'e'..n  n=3 print 'f'..n b(10) print 'g'..n end

    // print 'a'..n
    // b(9)
    // print 'd'..n
    // a()
    // print 'h'.. n
    // "#;

    // thrice
    //     let source_in = r#"

    // function thrice(fn)
    // for i = 1,  1 do
    //     fn(i)
    // end
    // end

    // thrice(function(i) print("p "..i) end)
    // "#;

    //     let source_in = r#"
    //     function create_counter()
    //     local count = 0
    //     return function()
    //         count = count + 1
    //         print(count)
    //         return count
    //     end
    // end

    // local counter = create_counter()
    // counter()
    // counter()
    //     "#;
    // let source_in = r#"
    // function echo(n)
    // print("x"..n)
    // return 2
    // end

    // echo(2)
    // print("y"..2)

    // --print(echo(echo(1)+echo(2)) + echo(echo(4)+echo(5)) )
    // "#;
    // let source_in = r#" 4*-6*(3 * 7 + 5) + 2 * 9"#;
    let source_in = r#"!(5-4 > 3*2 == !nil)"#;
    let source_in = "!nil==!false";
    let source_in = "local a= -5+6"; // sprint 1+8";

    //     let source_in = r#"
    //     do
    //     local a=1
    //     if true then
    //         local b=2
    //         print(a)
    //     end
    //     print(b)
    // end
    //     "#;

    //     let source_in = r#"
    // "#;

    // panic!("test");

    // fibonaci
    // let source_in = r#"
    // a=0
    // b=1
    // while a < 10000 do
    //     b+=temp
    //     print a
    //     temp=a
    //     a=b
    // end
    // "#;

    let mut global = environment::Environment::new();
    global.load_standard_library();

    // cli(source_in, &mut global);

    // match cli(source_in, &mut global) {
    //     value::Value::Nil => {}
    //     v => println!("{}", v),
    // }
    // println!(">");

    // loop {
    //     let mut line = String::new();
    //     if let Ok(_) = std::io::stdin().read_line(&mut line) {
    //         let s = line.trim();
    //         match cli(s, &mut global) {
    //             value::Value::Nil => {}
    //             v => println!("{}", v),
    //         }
    //         println!(">");
    //     }
    // }

    let mut compiler = Compiler::new();
    let c = compiler.compile(source_in.to_string(), &mut global);

    c.print_chunk();
    println!("-----------------");
    let mut vm = VM::new();
    if let Err(e) = vm.interpret(&c) {
        println!("{}", e);
    }
}

fn cli(source: &str, global: &mut environment::Environment) -> value::Value {
    println!("-----------------");
    let mut lexer = lexer::Lexer::new(source.to_owned());
    let mut t: Vec<TokenResult> = lexer.collect();
    let mut erronious = false;
    let mut tokens = vec![];
    let mut locations = vec![];
    t.drain(..).enumerate().for_each(|(i, res)| match res {
        Ok(t) => {
            let p = t.1;
            println!("|{}:{}| {}", p.0, p.1, t.0);

            tokens.push(t.0);
            locations.push(t.1);
        }
        Err(e) => {
            erronious = true;
            println!("|!! {}", e)
        }
    });

    if !erronious {
        let mut parser = crate::parser::parser::Parser::new(tokens, locations, global);
        println!("|----------------");
        let mut statements = parser.parse();
        statements
            .iter()
            .enumerate()
            .for_each(|(i, e)| println!("| {} {}", i, e));
        // println!("{}", exp);
        // let val = parser.evaluate(exp);
        let err = parser.get_errors();
        if err.len() > 0 {
            println!("|----------------");
            println!("Parse Errors:");
            err.iter().for_each(|e| println!("{}", e));
            println!("-----------------");
        } else {
            println!("-----------------");
            let mut resolver = crate::resolver::Resolver::new();
            resolver.process(&mut statements);
            let res = crate::interpreter::execute(global, &statements);
            match res {
                Ok(v) => {
                    println!("");
                    return v;
                }
                Err(e) => {
                    println!("| Runtime Errors:");
                    println!("| {}", e);
                    println!("-----------------");
                }
            }
        }
    }

    // println!("-----------------");
    // match val {
    //     Ok(v) => println!("{}", v),
    //     Err(e) => println!("Error: {}", e),
    // }
    // println!("-----------------");
    println!("");
    value::Value::Nil
}

#[cfg(test)]
mod tests {
    use crate::{
        chunk::Chunk,
        code::{self, OpCode},
        parser,
        token::Token,
        value::Value,
        vm::VM,
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
        println!(
            "size of statement: {}",
            size_of::<crate::statement::Statement>()
        );
        println!(
            "size of expression: {}",
            size_of::<crate::expression::Expression>()
        );
        println!(
            "size of option expression: {}",
            size_of::<Option<crate::expression::Expression>>()
        );
        println!(
            "size of boxed statement: {}",
            size_of::<Box<crate::statement::Statement>>()
        );
        println!(
            "size of boxed expression: {}",
            size_of::<Box<crate::expression::Expression>>()
        );
        println!(
            "size of vec expression: {}",
            size_of::<Vec<crate::expression::Expression>>()
        );
        println!(
            "size of native function: {}",
            size_of::<
                fn(
                    &mut crate::environment::Environment,
                    Vec<crate::value::Value>,
                ) -> crate::value::Value,
            >()
        );
        //Rc<Function>
        println!(
            "size of function: {}",
            size_of::<std::rc::Rc<crate::function::Function>>()
        );
        println!("size of value: {}", size_of::<crate::value::Value>());
        println!("size of OpCode: {}", size_of::<crate::code::OpCode>());
        println!(
            "size of environment: {}",
            size_of::<crate::environment::Environment>()
        );

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
    elapsed
    "#;
        let n = if let crate::value::Value::Number(n) = crate::cli(
            source_in,
            &mut crate::environment::Environment::new_with_std(),
        ) {
            println!("{} seconds", n);
            n
        } else {
            999999.
        };
        assert!(n < 2.14)
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
      elapsed
    "#;
        let n = if let crate::value::Value::Number(n) = crate::cli(
            source_in,
            &mut crate::environment::Environment::new_with_std(),
        ) {
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
        // c.write_code(
        //     OpCode::LITERAL {
        //         dest: 8,
        //         literal: 8,
        //     },
        //     (1, 0),
        // );
        c.write_value(Value::Number(1.2), (1, 1));
        c.write_value(Value::Number(3.4), (1, 2));
        c.write_code(OpCode::ADD, (1, 3));
        c.write_value(Value::Number(9.2), (1, 4));
        c.write_code(OpCode::DIVIDE, (1, 5));
        c.write_code(OpCode::NEGATE, (1, 1));
        c.write_code(OpCode::RETURN, (1, 3));
        c.print_chunk();
        println!("-----------------");
        let mut vm = VM::new();
        if let Err(e) = vm.interpret(&c) {
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
}
