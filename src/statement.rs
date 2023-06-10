use crate::expression::Expression;

pub enum Statement {
    Expression(Expression),
    Print(Expression),
    Declare { ident: String, value: Expression },
    // Var(Token, Expression),
    Block(Vec<Statement>),
    // If(Expression, Box<Statement>, Option<Box<Statement>>),
    // While(Expression, Box<Statement>),
    // Break,
    // Continue,
    // Function(Token, Vec<Token>, Box<Statement>),
    // Return(Token, Option<Expression>),
    // Class(Token, Option<Expression>, Vec<Statement>),
    // Import(Token, Option<Token>),
    // EOF,
    InvalidStatement,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Statement::Expression(expr) => write!(f, "$${}", expr),
            Statement::Print(expr) => write!(f, "$print$ {}", expr),
            Statement::Declare { ident, value } => write!(f, "$assign$ {} := {}", ident, value),
            Statement::InvalidStatement => write!(f, "!invalid!"),
            Statement::Block(statements) => {
                let mut s = String::new();
                for statement in statements {
                    s.push_str(&format!("|{}\n", statement));
                }
                write!(f, "$block$ {}", s)
            }
        }
    }
}
