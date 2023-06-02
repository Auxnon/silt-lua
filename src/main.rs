mod error;
mod lexer;
mod parser;
mod token;
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
    let source = "6+5/3-1%2";

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

    let exp = crate::parser::Parser::new(t, p).parse();
    println!("{}", exp);
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
