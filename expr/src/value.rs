use std::collections::HashMap;

use crate::ast::Expr;
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
    pub cmp: fn(&Value, &Value) -> std::cmp::Ordering,
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

pub fn value_eq(lhs: &Value, rhs: &Value) -> Result<bool, EvalError> {
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
        (Value::Interval(_), Value::Interval(_)) | (Value::Lambda(_), Value::Lambda(_)) => {
            Err(EvalError::EqMismatch {
                left_ty: lhs.type_tag(),
                right_ty: rhs.type_tag(),
                span: Span::default(),
            })
        }
        (a, b) => Err(EvalError::EqMismatch {
            left_ty: a.type_tag(),
            right_ty: b.type_tag(),
            span: Span::default(),
        }),
    }
}

pub fn contains_value(haystack: &[Value], needle: &Value) -> Result<bool, EvalError> {
    for value in haystack {
        if value_eq(value, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}
