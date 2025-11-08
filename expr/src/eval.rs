use crate::ast::Expr;
use crate::error::EvalError;
use crate::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Object(String),
}

pub trait Env {
    fn get_ident(&self, name: &str) -> Option<Value>;
    fn get_field(&self, base: &Value, field: &str) -> Option<Value>;
}

pub fn eval(_expr: &Expr, _env: &impl Env) -> Result<Value, EvalError> {
    Err(EvalError::Other {
        message: "eval not implemented".into(),
        span: Span::default(),
    })
}
