// expr/src/error.rs
#[derive(Debug, Clone)]
pub enum LexError {
    UnterminatedString { start: usize, end: usize },
    UnknownChar { ch: char, start: usize, end: usize },
}

#[derive(Debug, Clone)]
pub enum ParseError {
    UnexpectedToken(String),
    UnexpectedEOF,
    Other(String),
}

#[derive(Debug, Clone)]
pub enum EvalError {
    TypeMismatch(String),
    UnknownIdent(String),
    UnknownField { base: String, field: String },
    DivideByZero,
    Other(String),
}
