mod lexer;
mod token;
fn main() {
    // println!("Hello, world!");
    let source = r#"
    abc
    abc2
    bc=5--hi there
    a=1
    b=2
    c=a+b
    d='hello'
    e="world"
    f=[[multi 
    line]]
    print(c)
    "#;

    let sourceIter = source.chars().peekable();

    let mut lexer = lexer::Lexer {
        source: source.to_string(),
        start: 0,
        column: 0,
        current: 0,
        end: source.len(),
        line: 1,
        iterator: sourceIter,
        error_out: None,
        tokens: Vec::new(),
    };
    let t = lexer.parse();
    t.iter().for_each(|t| println!("{}", t));

    // match lexer.get_error() {
    //     Some(s) => println!("ERROR {}", s),
    //     None => println!("No error"),
    // }
    if let Some(s) = lexer.get_error() {
        println!("ERROR {}", s);
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
