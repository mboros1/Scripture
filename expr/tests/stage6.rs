use std::collections::HashMap;

use expr::{eval_exists, eval_forall, tokenize, Env, EvalError, Parser, Value};

#[derive(Default)]
struct TestEnv {
    sets: HashMap<String, Vec<Value>>,
}

impl TestEnv {
    fn with_set(mut self, name: &str, values: Vec<Value>) -> Self {
        self.sets.insert(name.to_string(), values);
        self
    }
}

impl Env for TestEnv {
    fn get_ident(&self, _name: &str) -> Option<Value> {
        None
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

    fn finite_set(&self, name: &str) -> Option<Vec<Value>> {
        self.sets.get(name).cloned()
    }
}

fn parse_expr(src: &str) -> expr::Expr {
    let tokens = tokenize(src).expect("tokenize");
    let mut parser = Parser::new(&tokens);
    parser.parse_expr().expect("parse")
}

#[test]
fn forall_succeeds_when_all_true() {
    let predicate = parse_expr("x > 0");
    let env = TestEnv::default().with_set(
        "nums",
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
    );
    let result = eval_forall("x", "nums", &predicate, &env).expect("forall eval");
    assert!(result);
}

#[test]
fn exists_detects_truth() {
    let predicate = parse_expr("x == 2");
    let env = TestEnv::default().with_set(
        "nums",
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
    );
    let result = eval_exists("x", "nums", &predicate, &env).expect("exists eval");
    assert!(result);
}

#[test]
fn quantifier_without_collection_errors() {
    let predicate = parse_expr("x > 0");
    let err = eval_forall("x", "missing", &predicate, &TestEnv::default()).expect_err("expected error");
    assert!(matches!(err, EvalError::CollectionUnknown { .. }));
}
