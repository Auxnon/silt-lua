use crate::expression::Expression;

pub enum Statement {
    Expression(Box<Expression>),
    Print(Box<Expression>),
    Declare {
        ident: usize,
        local: bool,
        value: Box<Expression>,
    },
    // Var(Token, Expression),
    Block(Vec<Statement>),
    If {
        cond: Box<Expression>,
        then: Vec<Statement>,
        else_cond: Option<Vec<Statement>>,
    },
    While {
        cond: Box<Expression>,
        block: Vec<Statement>,
    },
    NumericFor {
        ident: usize,
        start: Box<Expression>,
        end: Box<Expression>,
        step: Option<Box<Expression>>,
        block: Vec<Statement>,
    },
    // Break,
    // Continue,
    // Function {
    //     ident: usize,
    //     local: bool,
    //     args: Vec<usize>,
    //     block: Vec<Statement>,
    // },
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
            Statement::Declare {
                ident,
                local,
                value,
            } => write!(
                f,
                "$declare$ {} {} := {}",
                if *local { "local" } else { "global" },
                ident,
                value
            ),
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
            Statement::While { cond, block } => {
                let mut s = String::new();
                for statement in block {
                    s.push_str(&format!("\n||{}", statement));
                }
                write!(f, "$while$ {} {}", cond, s)
            }
            Statement::NumericFor {
                ident,
                start,
                end,
                step,
                block,
            } => {
                let mut s = String::new();
                for statement in block {
                    s.push_str(&format!("\n||{}", statement));
                }
                write!(
                    f,
                    "$for$ {} := {} to {} step {}",
                    ident,
                    start,
                    end,
                    match step {
                        Some(step) => format!("{}", step),
                        None => format!("1"),
                    }
                )
            } // Statement::Function {
              //     ident,
              //     local,
              //     args,
              //     block,
              // } => {
              //     let mut s = String::new();
              //     for statement in block {
              //         s.push_str(&format!("\n||{}", statement));
              //     }
              //     write!(
              //         f,
              //         "$function$ {} {} {}",
              //         if *local { "local" } else { "global" },
              //         ident,
              //         s
              //     )
              // }
        }
    }
}
