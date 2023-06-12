use crate::expression::Expression;

pub enum Statement {
    Expression(Expression),
    Print(Expression),
    Declare {
        ident: String,
        value: Expression,
    },
    // Var(Token, Expression),
    Block(Vec<Statement>),
    If {
        cond: Box<Expression>,
        then: Vec<Statement>,
        else_cond: Option<Vec<Statement>>,
    },
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
            Statement::Declare { ident, value } => write!(f, "$declare$ {} := {}", ident, value),
            Statement::InvalidStatement => write!(f, "!invalid!"),
            Statement::Block(statements) => {
                let mut s = String::new();
                for statement in statements {
                    s.push_str(&format!("\n||{}", statement));
                }
                write!(f, "$block$ {}", s)
            }
            Statement::If {
                cond,
                then,
                else_cond,
            } => {
                let mut s = String::new();
                for statement in then {
                    s.push_str(&format!("\n||{}", statement));
                }

                let mut s = format!("$if$ {} then {}", cond, s);

                if let Some(else_cond) = else_cond {
                    let mut s2 = String::new();
                    for statement in then {
                        s2.push_str(&format!("\n||{}", statement));
                    }
                    s.push_str(&format!(" else {}", s2));
                }
                write!(f, "{}", s)
            }
        }
    }
}
