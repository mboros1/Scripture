// expr/src/ast.rs
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Bool(bool),
    Str(String),
    Ident(IdentPath),

    EnumVariant { ty: String, variant: String },
    SetLiteral(Vec<Expr>),
    In { elem: Box<Expr>, set: Box<Expr> },
    SetOp { op: SetOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Cardinality(Box<Expr>),
    Interval(IntervalExpr),
    Infinity(InfinityKind),
    Lambda { params: Vec<String>, body: Box<Expr> },
    Call(Call),
    Pipe { lhs: Box<Expr>, call: Call },
    WithSemiring { name: String, body: Box<Expr> },

    Unary { op: UOp, expr: Box<Expr> },
    Binary { left: Box<Expr>, op: BOp, right: Box<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntervalExpr {
    pub lo: BoundExpr,
    pub hi: BoundExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundExpr {
    Open(Box<Expr>),
    Closed(Box<Expr>),
    NegInf,
    PosInf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfinityKind {
    Negative,
    Positive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub func: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetOp {
    Union,
    Intersect,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UOp {
    Not,
    Neg,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IdentPath(pub Vec<String>);
