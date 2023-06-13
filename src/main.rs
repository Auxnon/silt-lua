mod environment;
mod error;
mod expression;
mod interpreter;
mod lexer;
mod parser;
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
    // "#;
    let source_in = r#"
    a=1
    while a < 10_000_00 do
        a = a + 1
    end
    print a
    "#;
    let mut global = environment::Environment::new();
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
        assert!(size_of::<Token>() == 4);
    }
}
