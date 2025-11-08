// expr/src/ast.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Str(String),
    Ident(IdentPath),

    Unary { op: UOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BOp, right: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BOp {
    // arithmetic
    Add, Sub, Mul, Div, Rem,
    // comps
    Lt, Le, Gt, Ge,
    // equality
    Eq, Ne,
    // logical operators
    And, Or,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdentPath(pub Vec<String>);
