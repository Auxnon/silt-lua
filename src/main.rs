mod lexer;
mod token;
fn main() {
    // println!("Hello, world!");
    let source = r#"
    abc
    abc2
    2bc
    a=1
    b=2
    c=a+b
    print(c)
    "#;

    let sourceIter = source.chars().peekable();

    let mut lexer = lexer::Lexer {
        source: source.to_string(),
        start: 0,
        current: 0,
        end: source.len(),
        line: 1,
        iterator: sourceIter,
    };
    lexer.for_each(|token| {
        println!("{}", token);
    });
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
