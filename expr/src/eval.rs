use crate::ast::Expr;
use crate::error::EvalError;

pub trait Env {}

pub fn eval(_expr: &Expr, _env: &impl Env) -> Result<(), EvalError> {
    Ok(())
}