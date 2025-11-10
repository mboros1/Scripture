use crate::ast::Expr;
use crate::error::{EvalError, TypeTag};
use crate::eval::eval;
use crate::runtime::Env;
use crate::token::Span;
use crate::value::Value;

pub fn eval_forall(var: &str, collection: &str, predicate: &Expr, env: &dyn Env) -> Result<bool, EvalError> {
    let items = env
        .finite_set(collection)
        .ok_or(EvalError::CollectionUnknown { name: collection.to_string(), span: Span::default() })?;
    for value in items {
        let scope = QuantifierEnv::new(var, value, env);
        let result = eval(predicate, &scope)?;
        match result {
            Value::Bool(true) => continue,
            Value::Bool(false) => return Ok(false),
            other => {
                return Err(EvalError::QuantifierTypeMismatch {
                    quantifier: "forall",
                    found: other.type_tag(),
                    span: Span::default(),
                })
            }
        }
    }
    Ok(true)
}

pub fn eval_exists(var: &str, collection: &str, predicate: &Expr, env: &dyn Env) -> Result<bool, EvalError> {
    let items = env
        .finite_set(collection)
        .ok_or(EvalError::CollectionUnknown { name: collection.to_string(), span: Span::default() })?;
    for value in items {
        let scope = QuantifierEnv::new(var, value, env);
        let result = eval(predicate, &scope)?;
        match result {
            Value::Bool(true) => return Ok(true),
            Value::Bool(false) => continue,
            other => {
                return Err(EvalError::QuantifierTypeMismatch {
                    quantifier: "exists",
                    found: other.type_tag(),
                    span: Span::default(),
                })
            }
        }
    }
    Ok(false)
}

struct QuantifierEnv<'a> {
    var: String,
    value: Value,
    parent: &'a dyn Env,
}

impl<'a> QuantifierEnv<'a> {
    fn new(var: &str, value: Value, parent: &'a dyn Env) -> Self {
        Self {
            var: var.to_string(),
            value,
            parent,
        }
    }
}

impl<'a> Env for QuantifierEnv<'a> {
    fn get_ident(&self, name: &str) -> Option<Value> {
        if name == self.var {
            Some(self.value.clone())
        } else {
            self.parent.get_ident(name)
        }
    }

    fn get_field(&self, base: &Value, field: &str) -> Option<Value> {
        self.parent.get_field(base, field)
    }

    fn enum_info(&self, type_id: &str) -> Option<crate::value::EnumInfo> {
        self.parent.enum_info(type_id)
    }

    fn default_order(&self, type_id: &str) -> Option<crate::value::OrderInfo> {
        self.parent.default_order(type_id)
    }

    fn named_order(&self, order_id: &str) -> Option<crate::value::OrderInfo> {
        self.parent.named_order(order_id)
    }

    fn semiring(&self, name: &str) -> Option<crate::value::SemiringOps> {
        self.parent.semiring(name)
    }

    fn lattice_for(&self, ty: &TypeTag) -> Option<crate::value::LatticeOps> {
        self.parent.lattice_for(ty)
    }

    fn finite_set(&self, name: &str) -> Option<Vec<Value>> {
        self.parent.finite_set(name)
    }

    fn current_semiring(&self) -> Option<crate::value::SemiringOps> {
        self.parent.current_semiring()
    }
}
