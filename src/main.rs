mod error;
mod expression;
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
    while (true) {
        let mut line = String::new();
        if let Ok(_) = std::io::stdin().read_line(&mut line) {
            // let source = "1/0"; //6+5/3-6";
            let source = line.trim();
            println!("-----------------");

            let mut lexer = lexer::Lexer::new(source.to_owned());
            // println!("step 1");
            let (t, p) = lexer.parse();
            // println!("step 2");
            t.iter().for_each(|t| println!("{}", t));

            // match lexer.get_error() {
            //     Some(s) => println!("ERROR {}", s),
            //     None => println!("No error"),
            // }
            lexer
                .get_errors()
                .iter()
                .for_each(|e| println!("Err!!{}", e));

            let mut parser = crate::parser::parser::Parser::new(t, p);
            let exp = parser.parse();
            println!("{}", exp);
            let val = parser.evaluate(exp);
            println!("-----------------");
            match val {
                Ok(v) => println!("{}", v),
                Err(e) => println!("Error: {}", e),
            }
            println!("-----------------");
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
