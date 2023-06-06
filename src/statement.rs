use crate::{expression::Expression, token::Token};

pub enum Statement {
    Expression(Expression),
    Print(Expression),
    Var(Token, Expression),
    Block(Vec<Statement>),
    If(Expression, Box<Statement>, Option<Box<Statement>>),
    While(Expression, Box<Statement>),
    Break,
    Continue,
    Function(Token, Vec<Token>, Box<Statement>),
    Return(Token, Option<Expression>),
    Class(Token, Option<Expression>, Vec<Statement>),
    Import(Token, Option<Token>),
    EOF,
    InvalidStatement,
}
