mod environment;
mod error;
mod expression;
mod interpreter;
mod lexer;
mod parser;
mod standard;
mod statement;
mod token;
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

    let source_in = r#"
    i=1
    a="a"
    while i < 100_000 do
        i = i +1
        a = a .. "1"
    end
    print "done"
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
    let mut meth = |source: &str| {
        // let source = "1/0"; //6+5/3-6";
        println!("-----------------");

        let mut lexer = lexer::Lexer::new(source.to_owned());
        // println!("step 1");
        let (t, p) = lexer.parse();
        // println!("step 2");
        t.iter().enumerate().for_each(|(i, t)| {
            let p = p.get(i).unwrap_or(&(0, 0));
            println!("|{}:{}| {}", p.0, p.1, t)
        });

        // match lexer.get_error() {
        //     Some(s) => println!("ERROR {}", s),
        //     None => println!("No error"),
        // }
        lexer
            .get_errors()
            .iter()
            .for_each(|e| println!("| Err!!{}", e));

        let mut parser = crate::parser::parser::Parser::new(t, p, &mut global);
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
            let errs = crate::interpreter::execute(&mut global, &statements);
            if errs.len() > 0 {
                println!("Runtime Errors:");
                errs.iter().for_each(|e| println!("{}", e));
            }
        }

        // println!("-----------------");
        // match val {
        //     Ok(v) => println!("{}", v),
        //     Err(e) => println!("Error: {}", e),
        // }
        // println!("-----------------");
        println!("");
    };
    meth(source_in);

    while (true) {
        let mut line = String::new();
        if let Ok(_) = std::io::stdin().read_line(&mut line) {
            let s = line.trim();
            meth(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::token::Token;
    use std::mem::size_of;

    #[test]
    fn test_32bits() {
        println!("size of i32: {}", size_of::<i32>());
        println!("size of f32: {}", size_of::<f32>());
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
        println!("size of value: {}", size_of::<crate::value::Value>());
        println!(
            "size of environment: {}",
            size_of::<crate::environment::Environment>()
        );

        assert!(size_of::<Token>() == 24); //ideally 4 but whatever
    }
}
