use crate::expression::Expression;

pub enum Statement {
    Expression(Expression),
    Print(Expression),
    Assign { ident: String, value: Expression },
    // Var(Token, Expression),
    // Block(Vec<Statement>),
    // If(Expression, Box<Statement>, Option<Box<Statement>>),
    // While(Expression, Box<Statement>),
    // Break,
    // Continue,
    // Function(Token, Vec<Token>, Box<Statement>),
    // Return(Token, Option<Expression>),
    // Class(Token, Option<Expression>, Vec<Statement>),
    // Import(Token, Option<Token>),
    // EOF,
    // InvalidStatement,
}

impl std::fmt::Display for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Statement::Expression(expr) => write!(f, "$${}", expr),
            Statement::Print(expr) => write!(f, "$print$ {}", expr),
            Statement::Assign { ident, value } => write!(f, "$assign$ {} := {}", ident, value),
        }
    }
}
