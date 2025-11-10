use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, BOp, UOp, IdentPath, SetOp, IntervalExpr, BoundExpr};
use crate::builtins;
use crate::error::{EvalError, TypeTag};
use crate::runtime::Env;
use crate::token::Span;
use crate::value::{
    contains_value,
    value_eq,
    BoundValue,
    EnumInfo,
    IntervalValue,
    LatticeOps,
    LambdaValue,
    OrderInfo,
    SemiringOps,
    SetValue,
    Value,
};

pub fn eval(expr: &Expr, env: &dyn Env) -> Result<Value, EvalError> {
    eval_expr(expr, env)
}

fn eval_expr(expr: &Expr, env: &dyn Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Int(n) => Ok(Value::Int(*n)),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::EnumVariant { ty, variant } => resolve_enum_variant(ty, variant, env),
        Expr::Ident(path) => resolve_ident(path, env),
        Expr::Unary { op, expr } => {
            let value = eval_expr(expr, env)?;
            match op {
                UOp::Not => match value {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    other => Err(EvalError::TypeMismatch {
                        op: "!",
                        left: other.type_tag(),
                        right: TypeTag::Bool,
                        span: Span::default(),
                    }),
                },
                UOp::Neg => match value {
                    Value::Int(i) => Ok(Value::Int(-i)),
                    other => Err(EvalError::TypeMismatch {
                        op: "-",
                        left: other.type_tag(),
                        right: TypeTag::Int,
                        span: Span::default(),
                    }),
                },
            }
        }
        Expr::Binary { left, op, right } => {
            let lhs = eval_expr(left, env)?;
            let rhs = eval_expr(right, env)?;
            eval_binary(op, lhs, rhs, env)
        }
        Expr::SetLiteral(elements) => eval_set_literal(elements, env),
        Expr::In { elem, set } => {
            let value = eval_expr(elem, env)?;
            let set_value = eval_expr(set, env)?;
            eval_membership(value, set_value, env)
        }
        Expr::SetOp { op, lhs, rhs } => {
            let left_val = eval_expr(lhs, env)?;
            let right_val = eval_expr(rhs, env)?;
            eval_set_op(*op, left_val, right_val)
        }
        Expr::Cardinality(expr) => {
            let value = eval_expr(expr, env)?;
            match value {
                Value::Set(set) => Ok(Value::Int(set.elements.len() as i64)),
                other => Err(EvalError::InOnNonSet { rhs_ty: other.type_tag(), span: Span::default() }),
            }
        }
        Expr::Interval(interval) => eval_interval_literal(interval, env),
        Expr::Infinity(_) => Err(EvalError::IntervalInfiniteUnsupported {
            ty: TypeTag::Unknown,
            span: Span::default(),
        }),
        Expr::Lambda { params, body } => build_lambda(params, body, env),
        Expr::Call(call) => {
            let args = eval_args(&call.args, env)?;
            builtins::apply(&call.func, args, env, &mut |lambda, values, env| call_lambda(lambda, values, env))
        }
        Expr::Pipe { lhs, call } => {
            let lefthand = eval_expr(lhs, env)?;
            let mut args = vec![lefthand];
            args.extend(eval_args(&call.args, env)?);
            builtins::apply(&call.func, args, env, &mut |lambda, values, env| call_lambda(lambda, values, env))
        }
        Expr::WithSemiring { name, body } => {
            let ops = env
                .semiring(name)
                .ok_or(EvalError::SemiringMissing { span: Span::default() })?;
            let scope = SemiringScopeEnv { parent: env, current: ops };
            eval_expr(body, &scope)
        }
    }
}

fn resolve_ident(path: &IdentPath, env: &dyn Env) -> Result<Value, EvalError> {
    let mut iter = path.0.iter();
    let first = iter
        .next()
        .ok_or_else(|| EvalError::Other { message: "empty identifier path".into(), span: Span::default() })?;
    let mut current = env
        .get_ident(first)
        .ok_or_else(|| EvalError::UnknownIdent { name: first.clone(), span: Span::default() })?;
    for field in iter {
        current = env
            .get_field(&current, field)
            .ok_or_else(|| EvalError::UnknownField { base: format!("{:?}", current.type_tag()), field: field.clone(), span: Span::default() })?;
    }
    Ok(current)
}

fn resolve_enum_variant(ty: &str, variant: &str, env: &dyn Env) -> Result<Value, EvalError> {
    match env.enum_info(ty) {
        Some(info) => {
            if info.variants.iter().any(|v| v == variant)
                || info.aliases.iter().any(|(alias, canonical)| alias == variant && info.variants.iter().any(|v| v == canonical))
            {
                Ok(Value::Enum { type_id: info.type_id, variant: variant.to_string() })
            } else {
                Err(EvalError::EnumUnknownVariant {
                    type_id: ty.to_string(),
                    variant: variant.to_string(),
                    span: Span::default(),
                })
            }
        }
        None => Err(EvalError::EnumUnknownType { type_id: ty.to_string(), span: Span::default() }),
    }
}

fn eval_set_literal(elements: &[Expr], env: &dyn Env) -> Result<Value, EvalError> {
    let mut evaluated = Vec::with_capacity(elements.len());
    let mut elem_type: Option<TypeTag> = None;
    for expr in elements {
        let value = eval_expr(expr, env)?;
        let tag = value.type_tag();
        if let Some(expected) = &elem_type {
            if *expected != tag && tag != TypeTag::Unknown {
                return Err(EvalError::SetElemMismatch {
                    expected: expected.clone(),
                    found: tag,
                    span: Span::default(),
                });
            }
        } else if tag != TypeTag::Unknown {
            elem_type = Some(tag.clone());
        }
        evaluated.push(value);
    }
    let elem_type = elem_type.unwrap_or(TypeTag::Unknown);
    Ok(Value::Set(SetValue { elem_type, elements: evaluated }))
}

fn eval_membership(value: Value, set_value: Value, env: &dyn Env) -> Result<Value, EvalError> {
    match set_value {
        Value::Set(set) => {
            for elem in &set.elements {
                if value_eq(elem, &value)? {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        Value::Interval(interval) => {
            let contains = interval_contains(&value, interval.as_ref(), env)?;
            Ok(Value::Bool(contains))
        }
        other => Err(EvalError::InOnNonSet { rhs_ty: other.type_tag(), span: Span::default() }),
    }
}

fn eval_args(args: &[Expr], env: &dyn Env) -> Result<Vec<Value>, EvalError> {
    args.iter().map(|expr| eval_expr(expr, env)).collect()
}

fn eval_set_op(op: SetOp, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
    match (lhs, rhs) {
        (Value::Set(left), Value::Set(right)) => {
            if left.elem_type != right.elem_type
                && left.elem_type != TypeTag::Unknown
                && right.elem_type != TypeTag::Unknown
            {
                return Err(EvalError::SetTypeMismatch {
                    lhs_ty: left.elem_type.clone(),
                    rhs_ty: right.elem_type.clone(),
                    span: Span::default(),
                });
            }
            let elem_type = if left.elem_type != TypeTag::Unknown {
                left.elem_type.clone()
            } else {
                right.elem_type.clone()
            };
            let mut result = Vec::new();
            match op {
                SetOp::Union => {
                    for elem in left.elements.iter().chain(right.elements.iter()) {
                        if !contains_value(&result, elem)? {
                            result.push(elem.clone());
                        }
                    }
                }
                SetOp::Intersect => {
                    for elem in &left.elements {
                        if contains_value(&right.elements, elem)? && !contains_value(&result, elem)? {
                            result.push(elem.clone());
                        }
                    }
                }
                SetOp::Diff => {
                    for elem in &left.elements {
                        if !contains_value(&right.elements, elem)? {
                            result.push(elem.clone());
                        }
                    }
                }
            }
            Ok(Value::Set(SetValue { elem_type, elements: result }))
        }
        (_left, right) => Err(EvalError::InOnNonSet {
            rhs_ty: right.type_tag(),
            span: Span::default(),
        }),
    }
}

fn eval_interval_literal(interval: &IntervalExpr, env: &dyn Env) -> Result<Value, EvalError> {
    let (lo, lo_tag) = eval_bound_value(&interval.lo, env)?;
    let (hi, hi_tag) = eval_bound_value(&interval.hi, env)?;
    let elem_type = resolve_interval_type(lo_tag, hi_tag)?;
    validate_interval_bounds(&lo, &hi, &elem_type, env)?;

    Ok(Value::Interval(Box::new(IntervalValue { elem_type, lo, hi })))
}

fn eval_bound_value(
    bound: &BoundExpr,
    env: &dyn Env,
) -> Result<(BoundValue, Option<TypeTag>), EvalError> {
    match bound {
        BoundExpr::Open(expr) => {
            let value = eval_expr(expr, env)?;
            let tag = value.type_tag();
            Ok((BoundValue::Open(Box::new(value)), Some(tag)))
        }
        BoundExpr::Closed(expr) => {
            let value = eval_expr(expr, env)?;
            let tag = value.type_tag();
            Ok((BoundValue::Closed(Box::new(value)), Some(tag)))
        }
        BoundExpr::NegInf => Ok((BoundValue::NegInf, None)),
        BoundExpr::PosInf => Ok((BoundValue::PosInf, None)),
    }
}

fn resolve_interval_type(
    lo: Option<TypeTag>,
    hi: Option<TypeTag>,
) -> Result<TypeTag, EvalError> {
    match (lo, hi) {
        (Some(a), Some(b)) => {
            if a == b {
                Ok(a)
            } else {
                Err(EvalError::IntervalTypeMismatch {
                    lo_ty: a,
                    hi_ty: b,
                    x_ty: TypeTag::Unknown,
                    span: Span::default(),
                })
            }
        }
        (Some(tag), None) | (None, Some(tag)) => Ok(tag),
        (None, None) => Ok(TypeTag::Int),
    }
}

fn validate_interval_bounds(
    lo: &BoundValue,
    hi: &BoundValue,
    ty: &TypeTag,
    env: &dyn Env,
) -> Result<(), EvalError> {
    match (lo, hi) {
        (BoundValue::PosInf, _) | (_, BoundValue::NegInf) => {
            Err(EvalError::IntervalInvalidBounds { span: Span::default() })
        }
        (BoundValue::NegInf, _) | (_, BoundValue::PosInf) => Ok(()),
        (BoundValue::Open(l), BoundValue::Open(r))
        | (BoundValue::Open(l), BoundValue::Closed(r))
        | (BoundValue::Closed(l), BoundValue::Open(r))
        | (BoundValue::Closed(l), BoundValue::Closed(r)) => {
            let ord = compare_values_with_type(l.as_ref(), r.as_ref(), ty, env)?;
            if ord == Ordering::Greater {
                Err(EvalError::IntervalInvalidBounds { span: Span::default() })
            } else {
                Ok(())
            }
        }
    }
}

fn interval_contains(
    value: &Value,
    interval: &IntervalValue,
    env: &dyn Env,
) -> Result<bool, EvalError> {
    let value_tag = value.type_tag();
    let target_ty = if interval.elem_type == TypeTag::Unknown {
        value_tag.clone()
    } else {
        interval.elem_type.clone()
    };

    if target_ty != TypeTag::Unknown && value_tag != target_ty {
        return Err(EvalError::IntervalTypeMismatch {
            lo_ty: interval.elem_type.clone(),
            hi_ty: interval.elem_type.clone(),
            x_ty: value_tag,
            span: Span::default(),
        });
    }

    let lower_ok = satisfies_lower_bound(value, &interval.lo, &target_ty, env)?;
    let upper_ok = satisfies_upper_bound(value, &interval.hi, &target_ty, env)?;
    Ok(lower_ok && upper_ok)
}

fn satisfies_lower_bound(
    value: &Value,
    bound: &BoundValue,
    ty: &TypeTag,
    env: &dyn Env,
) -> Result<bool, EvalError> {
    match bound {
        BoundValue::NegInf => Ok(true),
        BoundValue::PosInf => Err(EvalError::IntervalInvalidBounds { span: Span::default() }),
        BoundValue::Open(v) => Ok(compare_values_with_type(value, v.as_ref(), ty, env)? == Ordering::Greater),
        BoundValue::Closed(v) => {
            let ord = compare_values_with_type(value, v.as_ref(), ty, env)?;
            Ok(ord == Ordering::Greater || ord == Ordering::Equal)
        }
    }
}

fn satisfies_upper_bound(
    value: &Value,
    bound: &BoundValue,
    ty: &TypeTag,
    env: &dyn Env,
) -> Result<bool, EvalError> {
    match bound {
        BoundValue::PosInf => Ok(true),
        BoundValue::NegInf => Err(EvalError::IntervalInvalidBounds { span: Span::default() }),
        BoundValue::Open(v) => Ok(compare_values_with_type(value, v.as_ref(), ty, env)? == Ordering::Less),
        BoundValue::Closed(v) => {
            let ord = compare_values_with_type(value, v.as_ref(), ty, env)?;
            Ok(ord == Ordering::Less || ord == Ordering::Equal)
        }
    }
}

fn compare_values_with_type(
    a: &Value,
    b: &Value,
    ty: &TypeTag,
    env: &dyn Env,
) -> Result<Ordering, EvalError> {
    match ty {
        TypeTag::Int => match (a, b) {
            (Value::Int(lhs), Value::Int(rhs)) => Ok(lhs.cmp(rhs)),
            (left, right) => Err(EvalError::IntervalTypeMismatch {
                lo_ty: left.type_tag(),
                hi_ty: right.type_tag(),
                x_ty: ty.clone(),
                span: Span::default(),
            }),
        },
        TypeTag::Enum(type_id) => match (a, b) {
            (
                Value::Enum { type_id: left_ty, .. },
                Value::Enum { type_id: right_ty, .. },
            ) if left_ty == right_ty => {
                if let Some(order) = env.default_order(type_id) {
                    Ok((order.cmp)(a, b))
                } else {
                    Err(EvalError::OrderMissing { ty: ty.clone(), span: Span::default() })
                }
            }
            (left, right) => Err(EvalError::IntervalTypeMismatch {
                lo_ty: left.type_tag(),
                hi_ty: right.type_tag(),
                x_ty: ty.clone(),
                span: Span::default(),
            }),
        },
        _ => Err(EvalError::OrderMissing { ty: ty.clone(), span: Span::default() }),
    }
}

fn compare_ordered(
    lhs: Value,
    rhs: Value,
    env: &dyn Env,
    predicate: impl Fn(Ordering) -> bool,
) -> Result<Value, EvalError> {
    let ord = compare_values(&lhs, &rhs, env)?;
    Ok(Value::Bool(predicate(ord)))
}

fn compare_values(lhs: &Value, rhs: &Value, env: &dyn Env) -> Result<Ordering, EvalError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),
        (
            Value::Enum { type_id: ty_l, .. },
            Value::Enum { type_id: ty_r, .. },
        ) if ty_l == ty_r => {
            if let Some(order) = env.default_order(ty_l) {
                Ok((order.cmp)(lhs, rhs))
            } else {
                Err(EvalError::OrderMissing {
                    ty: TypeTag::Enum(ty_l.clone()),
                    span: Span::default(),
                })
            }
        }
        (a, b) => Err(EvalError::TypeMismatch {
            op: "cmp",
            left: a.type_tag(),
            right: b.type_tag(),
            span: Span::default(),
        }),
    }
}

fn eval_binary(op: &BOp, lhs: Value, rhs: Value, env: &dyn Env) -> Result<Value, EvalError> {
    match op {
        BOp::Add => match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "+",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Sub => match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "-",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Mul => match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "*",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Div => match (lhs, rhs) {
            (Value::Int(_), Value::Int(0)) => Err(EvalError::DivideByZero { span: Span::default() }),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a / b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "/",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Rem => match (lhs, rhs) {
            (Value::Int(_), Value::Int(0)) => Err(EvalError::DivideByZero { span: Span::default() }),
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "%",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Eq => {
            let eq = value_eq(&lhs, &rhs)?;
            Ok(Value::Bool(eq))
        }
        BOp::Ne => {
            let eq = value_eq(&lhs, &rhs)?;
            Ok(Value::Bool(!eq))
        }
        BOp::Lt => compare_ordered(lhs, rhs, env, |o| o == Ordering::Less),
        BOp::Le => compare_ordered(lhs, rhs, env, |o| o != Ordering::Greater),
        BOp::Gt => compare_ordered(lhs, rhs, env, |o| o == Ordering::Greater),
        BOp::Ge => compare_ordered(lhs, rhs, env, |o| o != Ordering::Less),
        BOp::And => match (lhs, rhs) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "&&",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
        BOp::Or => match (lhs, rhs) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
            (a, b) => Err(EvalError::TypeMismatch {
                op: "||",
                left: a.type_tag(),
                right: b.type_tag(),
                span: Span::default(),
            }),
        },
    }
}

struct SemiringScopeEnv<'a> {
    parent: &'a dyn Env,
    current: SemiringOps,
}

impl<'a> Env for SemiringScopeEnv<'a> {
    fn get_ident(&self, name: &str) -> Option<Value> {
        self.parent.get_ident(name)
    }

    fn get_field(&self, base: &Value, field: &str) -> Option<Value> {
        self.parent.get_field(base, field)
    }

    fn enum_info(&self, type_id: &str) -> Option<EnumInfo> {
        self.parent.enum_info(type_id)
    }

    fn default_order(&self, type_id: &str) -> Option<OrderInfo> {
        self.parent.default_order(type_id)
    }

    fn named_order(&self, order_id: &str) -> Option<OrderInfo> {
        self.parent.named_order(order_id)
    }

    fn semiring(&self, name: &str) -> Option<SemiringOps> {
        self.parent.semiring(name)
    }

    fn lattice_for(&self, ty: &TypeTag) -> Option<LatticeOps> {
        self.parent.lattice_for(ty)
    }

    fn finite_set(&self, name: &str) -> Option<Vec<Value>> {
        self.parent.finite_set(name)
    }

    fn current_semiring(&self) -> Option<SemiringOps> {
        Some(self.current.clone())
    }
}

fn build_lambda(params: &[String], body: &Expr, env: &dyn Env) -> Result<Value, EvalError> {
    let bound: HashSet<String> = params.iter().cloned().collect();
    let mut free = HashSet::new();
    collect_free_vars(body, &bound, &mut free);
    let mut captures = HashMap::new();
    for name in free {
        if let Some(value) = env.get_ident(&name) {
            captures.insert(name, value);
        } else {
            return Err(EvalError::LambdaCaptureUnknown { name, span: Span::default() });
        }
    }
    Ok(Value::Lambda(LambdaValue {
        params: params.to_vec(),
        body: Box::new(body.clone()),
        captures,
    }))
}

fn call_lambda(lambda: &LambdaValue, args: &[Value], env: &dyn Env) -> Result<Value, EvalError> {
    if lambda.params.len() != args.len() {
        return Err(EvalError::LambdaArity {
            expected: lambda.params.len(),
            found: args.len(),
            span: Span::default(),
        });
    }
    let mut scope_args = HashMap::new();
    for (name, value) in lambda.params.iter().zip(args.iter()) {
        scope_args.insert(name.clone(), value.clone());
    }
    let lambda_env = LambdaEnv {
        params: scope_args,
        captures: lambda.captures.clone(),
        parent: env,
    };
    eval_expr(&lambda.body, &lambda_env)
}

struct LambdaEnv<'a> {
    params: HashMap<String, Value>,
    captures: HashMap<String, Value>,
    parent: &'a dyn Env,
}

impl<'a> Env for LambdaEnv<'a> {
    fn get_ident(&self, name: &str) -> Option<Value> {
        self.params
            .get(name)
            .cloned()
            .or_else(|| self.captures.get(name).cloned())
            .or_else(|| self.parent.get_ident(name))
    }

    fn get_field(&self, base: &Value, field: &str) -> Option<Value> {
        self.parent.get_field(base, field)
    }

    fn enum_info(&self, type_id: &str) -> Option<EnumInfo> {
        self.parent.enum_info(type_id)
    }

    fn default_order(&self, type_id: &str) -> Option<OrderInfo> {
        self.parent.default_order(type_id)
    }

    fn named_order(&self, order_id: &str) -> Option<OrderInfo> {
        self.parent.named_order(order_id)
    }

    fn semiring(&self, name: &str) -> Option<SemiringOps> {
        self.parent.semiring(name)
    }

    fn lattice_for(&self, ty: &TypeTag) -> Option<LatticeOps> {
        self.parent.lattice_for(ty)
    }

    fn finite_set(&self, name: &str) -> Option<Vec<Value>> {
        self.parent.finite_set(name)
    }

    fn current_semiring(&self) -> Option<SemiringOps> {
        self.parent.current_semiring()
    }
}

fn collect_free_vars(expr: &Expr, bound: &HashSet<String>, acc: &mut HashSet<String>) {
    match expr {
        Expr::Int(_) | Expr::Bool(_) | Expr::Str(_) | Expr::EnumVariant { .. } | Expr::Infinity(_) => {}
        Expr::Ident(path) => note_ident(path, bound, acc),
        Expr::Unary { expr, .. } => collect_free_vars(expr, bound, acc),
        Expr::Binary { left, right, .. } => {
            collect_free_vars(left, bound, acc);
            collect_free_vars(right, bound, acc);
        }
        Expr::SetLiteral(items) => {
            for item in items {
                collect_free_vars(item, bound, acc);
            }
        }
        Expr::In { elem, set } => {
            collect_free_vars(elem, bound, acc);
            collect_free_vars(set, bound, acc);
        }
        Expr::SetOp { lhs, rhs, .. } => {
            collect_free_vars(lhs, bound, acc);
            collect_free_vars(rhs, bound, acc);
        }
        Expr::Cardinality(expr) => collect_free_vars(expr, bound, acc),
        Expr::Interval(interval) => {
            collect_free_vars_bound(&interval.lo, bound, acc);
            collect_free_vars_bound(&interval.hi, bound, acc);
        }
        Expr::Lambda { params, body } => {
            let mut inner_bound = bound.clone();
            for param in params {
                inner_bound.insert(param.clone());
            }
            collect_free_vars(body, &inner_bound, acc);
        }
        Expr::Call(call) => collect_free_vars_call(call, bound, acc),
        Expr::Pipe { lhs, call } => {
            collect_free_vars(lhs, bound, acc);
            collect_free_vars_call(call, bound, acc);
        }
        Expr::WithSemiring { body, .. } => collect_free_vars(body, bound, acc),
    }
}

fn collect_free_vars_call(call: &crate::ast::Call, bound: &HashSet<String>, acc: &mut HashSet<String>) {
    for arg in &call.args {
        collect_free_vars(arg, bound, acc);
    }
}

fn collect_free_vars_bound(bound_expr: &BoundExpr, bound: &HashSet<String>, acc: &mut HashSet<String>) {
    match bound_expr {
        BoundExpr::Open(expr) | BoundExpr::Closed(expr) => collect_free_vars(expr, bound, acc),
        BoundExpr::NegInf | BoundExpr::PosInf => {}
    }
}

fn note_ident(path: &IdentPath, bound: &HashSet<String>, acc: &mut HashSet<String>) {
    if let Some(head) = path.0.first() {
        if !bound.contains(head) {
            acc.insert(head.clone());
        }
    }
}
