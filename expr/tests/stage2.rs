use std::collections::HashMap;

use expr::{eval, tokenize, Env, EvalError, Parser, Value};

#[derive(Default)]
struct TestEnv {
    idents: HashMap<String, Value>,
}

impl TestEnv {
    fn with_ident(mut self, name: &str, value: Value) -> Self {
        self.idents.insert(name.to_string(), value);
        self
    }
}

impl Env for TestEnv {
    fn get_ident(&self, name: &str) -> Option<Value> {
        self.idents.get(name).cloned()
    }

    fn get_field(&self, _base: &Value, _field: &str) -> Option<Value> {
        None
    }

    fn enum_info(&self, _type_id: &str) -> Option<expr::EnumInfo> {
        None
    }

    fn default_order(&self, _type_id: &str) -> Option<expr::OrderInfo> {
        None
    }

    fn named_order(&self, _order_id: &str) -> Option<expr::OrderInfo> {
        None
    }
}

fn parse_expr(src: &str) -> expr::Expr {
    let tokens = tokenize(src).expect("tokenize");
    let mut parser = Parser::new(&tokens);
    parser.parse_expr().expect("parse")
}

#[test]
fn interval_membership_inclusive_lower_exclusive_upper() {
    let expr = parse_expr("x in [1, 3)");
    let env = TestEnv::default().with_ident("x", Value::Int(2));
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Bool(true));

    let env = TestEnv::default().with_ident("x", Value::Int(3));
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Bool(false));
}

#[test]
fn interval_with_infinities_covers_all_ints() {
    let expr = parse_expr("x in (-inf, +inf)");
    let env = TestEnv::default().with_ident("x", Value::Int(-10));
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn interval_invalid_bounds_error() {
    let expr = parse_expr("[5, 3]");
    let err = eval(&expr, &TestEnv::default()).expect_err("expected error");
    assert!(matches!(err, EvalError::IntervalInvalidBounds { .. }));
}

#[test]
fn interval_type_mismatch_error() {
    let expr = parse_expr("[1, true]");
    let err = eval(&expr, &TestEnv::default()).expect_err("expected error");
    assert!(matches!(err, EvalError::IntervalTypeMismatch { .. }));
}
