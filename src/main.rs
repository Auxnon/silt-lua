mod environment;
mod error;
mod expression;
mod function;
mod interpreter;
mod lexer;
mod parser;
mod standard;
mod statement;
mod token;
mod userdata;
mod value;
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

    let source_in = r#"

    n=1
    function b(h) print 'b'..n n=h print 'c'..n end

    function a() print 'e'..n n=3 print 'f'..n b(10) print 'g'..n end

    print 'a'..n
    b(9)
    print 'd'..n
    a()
    print 'h'.. n
    "#;

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

    cli(source_in, &mut global);
    println!(">");

    while (true) {
        let mut line = String::new();
        if let Ok(_) = std::io::stdin().read_line(&mut line) {
            let s = line.trim();
            match cli(s, &mut global) {
                value::Value::Nil => {}
                v => println!("{}", v),
            }
            println!(">");
        }
    }
}

fn cli(source: &str, global: &mut environment::Environment) -> value::Value {
    println!("-----------------");
    let mut lexer = lexer::Lexer::new(source.to_owned());
    let (t, p) = lexer.parse();
    t.iter().enumerate().for_each(|(i, t)| {
        let p = p.get(i).unwrap_or(&(0, 0));
        println!("|{}:{}| {}", p.0, p.1, t)
    });

    let errs = lexer.get_errors();
    if errs.len() > 0 {
        println!("|----------------");
        println!("Lexer Errors:");
        errs.iter().for_each(|e| println!("| {}", e));
        println!("-----------------");
    } else {
        let mut parser = crate::parser::parser::Parser::new(t, p, global);
        println!("|----------------");
        let statements = parser.parse();
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
    use crate::{parser, token::Token};
    use std::mem::size_of;

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
        println!("size of str: {}", size_of::<&str>());
        println!("size of String: {}", size_of::<String>());
        println!("size of Box<str>: {}", size_of::<Box<str>>());
        println!("size of Operator: {}", size_of::<crate::token::Operator>());
        println!("size of Flag: {}", size_of::<crate::token::Flag>());
        println!("size of token: {}", size_of::<Token>());
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
        println!(
            "size of environment: {}",
            size_of::<crate::environment::Environment>()
        );

        assert!(size_of::<Token>() == 24); //ideally 4 but whatever
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
    print "elapsed: "..elapsed
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
        assert!(n < 2.12)
    }
}
