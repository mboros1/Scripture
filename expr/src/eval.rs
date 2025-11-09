use std::cmp::Ordering;
use std::collections::HashMap;

use crate::ast::{Expr, BOp, UOp, IdentPath, SetOp, IntervalExpr, BoundExpr, Call};
use crate::error::{EvalError, TypeTag};
use crate::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Enum { type_id: String, variant: String },
    Set(SetValue),
    Interval(Box<IntervalValue>),
    Object(String),
    Unknown,
    Lambda(LambdaValue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetValue {
    pub elem_type: TypeTag,
    pub elements: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntervalValue {
    pub elem_type: TypeTag,
    pub lo: BoundValue,
    pub hi: BoundValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundValue {
    Open(Box<Value>),
    Closed(Box<Value>),
    NegInf,
    PosInf,
}

impl Value {
    pub fn type_tag(&self) -> TypeTag {
        match self {
            Value::Int(_) => TypeTag::Int,
            Value::Bool(_) => TypeTag::Bool,
            Value::Str(_) => TypeTag::Str,
            Value::Enum { type_id, .. } => TypeTag::Enum(type_id.clone()),
            Value::Set(set) => TypeTag::set(set.elem_type.clone()),
            Value::Interval(interval) => TypeTag::interval(interval.elem_type.clone()),
            Value::Object(_) => TypeTag::Unknown,
            Value::Unknown => TypeTag::Unknown,
            Value::Lambda(_) => TypeTag::Lambda,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub type_id: String,
    pub variants: Vec<String>,
    pub aliases: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub enum OrderKind {
    StrictWeak,
    Total,
}

#[derive(Clone, Debug)]
pub struct OrderInfo {
    pub order_id: String,
    pub type_id: String,
    pub kind: OrderKind,
    pub cmp: fn(&Value, &Value) -> Ordering,
}

#[derive(Clone, Debug)]
pub struct SemiringOps {
    pub name: String,
    pub zero: Value,
    pub one: Value,
    pub oplus: fn(&Value, &Value) -> Result<Value, EvalError>,
    pub otimes: fn(&Value, &Value) -> Result<Value, EvalError>,
    pub idempotent_oplus: bool,
    pub commutative_otimes: bool,
}

#[derive(Clone, Debug)]
pub struct LatticeOps {
    pub type_id: String,
    pub has_top: bool,
    pub has_bottom: bool,
    pub top: Option<Value>,
    pub bottom: Option<Value>,
    pub meet: fn(&Value, &Value) -> Result<Value, EvalError>,
    pub join: fn(&Value, &Value) -> Result<Value, EvalError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaValue {
    pub params: Vec<String>,
    pub body: Box<Expr>,
    pub captures: HashMap<String, Value>,
}

pub trait Env {
    fn get_ident(&self, name: &str) -> Option<Value>;
    fn get_field(&self, base: &Value, field: &str) -> Option<Value>;

    fn enum_info(&self, type_id: &str) -> Option<EnumInfo>;
    fn default_order(&self, type_id: &str) -> Option<OrderInfo>;
    fn named_order(&self, order_id: &str) -> Option<OrderInfo>;

    fn semiring(&self, _name: &str) -> Option<SemiringOps> {
        None
    }

    fn lattice_ops(&self, _type_id: &str) -> Option<LatticeOps> {
        None
    }

    fn capture_snapshot(&self) -> Vec<(String, Value)> {
        Vec::new()
    }
}

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
            eval_binary(op, lhs, rhs)
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
        Expr::Call(call) => eval_call(&call, env),
        Expr::Pipe { lhs, call } => {
            let lefthand = eval_expr(lhs, env)?;
            eval_pipe(&call, lefthand, env)
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

fn contains_value(haystack: &[Value], needle: &Value) -> Result<bool, EvalError> {
    for value in haystack {
        if value_eq(value, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
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
    _env: &dyn Env,
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
        _ => Err(EvalError::OrderMissing { ty: ty.clone(), span: Span::default() }),
    }
}

fn eval_binary(op: &BOp, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
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
        BOp::Lt => compare_ints(lhs, rhs, |o| o == Ordering::Less),
        BOp::Le => compare_ints(lhs, rhs, |o| o != Ordering::Greater),
        BOp::Gt => compare_ints(lhs, rhs, |o| o == Ordering::Greater),
        BOp::Ge => compare_ints(lhs, rhs, |o| o != Ordering::Less),
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

fn compare_ints(lhs: Value, rhs: Value, predicate: impl Fn(Ordering) -> bool) -> Result<Value, EvalError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Bool(predicate(a.cmp(&b)))),
        (a, b) => Err(EvalError::TypeMismatch {
            op: "cmp",
            left: a.type_tag(),
            right: b.type_tag(),
            span: Span::default(),
        }),
    }
}

fn value_eq(lhs: &Value, rhs: &Value) -> Result<bool, EvalError> {
    match (lhs, rhs) {
        (Value::Int(a), Value::Int(b)) => Ok(a == b),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::Str(a), Value::Str(b)) => Ok(a == b),
        (
            Value::Enum { type_id: ty_a, variant: var_a },
            Value::Enum { type_id: ty_b, variant: var_b },
        ) => {
            if ty_a != ty_b {
                Err(EvalError::EqMismatch {
                    left_ty: TypeTag::Enum(ty_a.clone()),
                    right_ty: TypeTag::Enum(ty_b.clone()),
                    span: Span::default(),
                })
            } else {
                Ok(var_a == var_b)
            }
        }
        (Value::Set(a), Value::Set(b)) => {
            if a.elements.len() != b.elements.len() {
                return Ok(false);
            }
            for elem in &a.elements {
                if !contains_value(&b.elements, elem)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Interval(_), Value::Interval(_)) => Err(EvalError::EqMismatch {
            left_ty: lhs.type_tag(),
            right_ty: rhs.type_tag(),
            span: Span::default(),
        }),
        (Value::Lambda(_), Value::Lambda(_)) => Err(EvalError::EqMismatch {
            left_ty: lhs.type_tag(),
            right_ty: rhs.type_tag(),
            span: Span::default(),
        }),
        (a, b) => Err(EvalError::EqMismatch {
            left_ty: a.type_tag(),
            right_ty: b.type_tag(),
            span: Span::default(),
        }),
    }
}

fn builtin_map(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let lambda = match &args[1] {
        Value::Lambda(func) => func.clone(),
        other => {
            return Err(EvalError::LambdaType {
                param: "map".into(),
                expected: TypeTag::Lambda,
                found: other.type_tag(),
                span: Span::default(),
            })
        }
    };

    let mut results = Vec::new();
    let mut elem_type = TypeTag::Unknown;
    for elem in &set.elements {
        let value = call_lambda(&lambda, &[elem.clone()], env)?;
        if elem_type == TypeTag::Unknown {
            elem_type = value.type_tag();
        }
        if !contains_value(&results, &value)? {
            results.push(value);
        }
    }
    Ok(Value::Set(SetValue { elem_type, elements: results }))
}

fn builtin_filter(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let lambda = match &args[1] {
        Value::Lambda(func) => func.clone(),
        other => {
            return Err(EvalError::LambdaType {
                param: "filter".into(),
                expected: TypeTag::Lambda,
                found: other.type_tag(),
                span: Span::default(),
            })
        }
    };

    let mut results = Vec::new();
    for elem in &set.elements {
        let value = call_lambda(&lambda, &[elem.clone()], env)?;
        match value {
            Value::Bool(true) => results.push(elem.clone()),
            Value::Bool(false) => {}
            other => {
                return Err(EvalError::TypeMismatch {
                    op: "filter",
                    left: other.type_tag(),
                    right: TypeTag::Bool,
                    span: Span::default(),
                })
            }
        }
    }
    Ok(Value::Set(SetValue { elem_type: set.elem_type, elements: results }))
}

fn builtin_fold(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 3 {
        return Err(EvalError::LambdaArity { expected: 3, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let mut acc = args[1].clone();
    let lambda = match &args[2] {
        Value::Lambda(func) => func.clone(),
        other => {
            return Err(EvalError::LambdaType {
                param: "fold".into(),
                expected: TypeTag::Lambda,
                found: other.type_tag(),
                span: Span::default(),
            })
        }
    };

    for elem in &set.elements {
        acc = call_lambda(&lambda, &[acc, elem.clone()], env)?;
    }
    Ok(acc)
}

fn builtin_reduce_meet(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::LambdaArity { expected: 1, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let lattice = env
        .lattice_ops(&match &set.elem_type {
            TypeTag::Enum(name) => name.clone(),
            TypeTag::Int => "core.Int".into(),
            other => format!("{:?}", other),
        })
        .ok_or(EvalError::LatticeRequired { ty: set.elem_type.clone(), span: Span::default() })?;

    let mut iter = set.elements.iter();
    let mut acc = iter
        .next()
        .cloned()
        .ok_or(EvalError::Other { message: "reduce_meet on empty set".into(), span: Span::default() })?;
    for elem in iter {
        acc = (lattice.meet)(&acc, elem)?;
    }
    Ok(acc)
}

fn builtin_reduce_join(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::LambdaArity { expected: 1, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let lattice = env
        .lattice_ops(&match &set.elem_type {
            TypeTag::Enum(name) => name.clone(),
            TypeTag::Int => "core.Int".into(),
            other => format!("{:?}", other),
        })
        .ok_or(EvalError::LatticeRequired { ty: set.elem_type.clone(), span: Span::default() })?;

    let mut iter = set.elements.iter();
    let mut acc = iter
        .next()
        .cloned()
        .ok_or(EvalError::Other { message: "reduce_join on empty set".into(), span: Span::default() })?;
    for elem in iter {
        acc = (lattice.join)(&acc, elem)?;
    }
    Ok(acc)
}

fn builtin_reduce_semiring(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = match &args[0] {
        Value::Set(set) => set.clone(),
        other => {
            return Err(EvalError::InOnNonSet {
                rhs_ty: other.type_tag(),
                span: Span::default(),
            })
        }
    };
    let lambda = match &args[1] {
        Value::Lambda(func) => func.clone(),
        other => {
            return Err(EvalError::LambdaType {
                param: "reduce_semiring".into(),
                expected: TypeTag::Lambda,
                found: other.type_tag(),
                span: Span::default(),
            })
        }
    };

    let semiring = env.semiring("default").ok_or(EvalError::SemiringMissing { span: Span::default() })?;
    let mut acc = semiring.zero.clone();
    for elem in &set.elements {
        let mapped = call_lambda(&lambda, &[elem.clone()], env)?;
        acc = (semiring.oplus)(&acc, &mapped)?;
    }
    Ok(acc)
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

    fn lattice_ops(&self, type_id: &str) -> Option<LatticeOps> {
        self.parent.lattice_ops(type_id)
    }

    fn capture_snapshot(&self) -> Vec<(String, Value)> {
        self.parent.capture_snapshot()
    }
}

fn build_lambda(params: &[String], body: &Expr, env: &dyn Env) -> Result<Value, EvalError> {
    let captures = env
        .capture_snapshot()
        .into_iter()
        .collect::<HashMap<_, _>>();
    Ok(Value::Lambda(LambdaValue {
        params: params.to_vec(),
        body: Box::new(body.clone()),
        captures,
    }))
}

fn eval_call(call: &Call, env: &dyn Env) -> Result<Value, EvalError> {
    let mut arg_values = Vec::with_capacity(call.args.len());
    for arg in &call.args {
        arg_values.push(eval_expr(arg, env)?);
    }
    apply_builtin(&call.func, arg_values, env)
}

fn eval_pipe(call: &Call, lhs: Value, env: &dyn Env) -> Result<Value, EvalError> {
    let mut arg_values = Vec::with_capacity(call.args.len() + 1);
    arg_values.push(lhs);
    for arg in &call.args {
        arg_values.push(eval_expr(arg, env)?);
    }
    apply_builtin(&call.func, arg_values, env)
}

fn apply_builtin(name: &str, args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    match name {
        "map" => builtin_map(args, env),
        "filter" => builtin_filter(args, env),
        "fold" => builtin_fold(args, env),
        "reduce_meet" => builtin_reduce_meet(args, env),
        "reduce_join" => builtin_reduce_join(args, env),
        "reduce_semiring" => builtin_reduce_semiring(args, env),
        _ => Err(EvalError::CallUnknown { func: name.into(), span: Span::default() }),
    }
}
