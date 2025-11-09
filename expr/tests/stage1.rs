use std::collections::HashMap;

use expr::{eval, tokenize, EnumInfo, Env, EvalError, Parser, Value};

#[derive(Default)]
struct TestEnv {
    idents: HashMap<String, Value>,
    enums: HashMap<String, EnumInfo>,
}

impl TestEnv {
    fn with_enum(mut self, info: EnumInfo) -> Self {
        self.enums.insert(info.type_id.clone(), info);
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

    fn enum_info(&self, type_id: &str) -> Option<EnumInfo> {
        self.enums.get(type_id).cloned()
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
fn enum_equality_succeeds() {
    let expr = parse_expr("core.Status::Ready == core.Status::Ready");
    let env = TestEnv::default().with_enum(EnumInfo {
        type_id: "core.Status".into(),
        variants: vec!["Ready".into(), "Pending".into()],
        aliases: vec![],
    });
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn enum_variant_unknown_type_errors() {
    let expr = parse_expr("foo.Status::Ready");
    let env = TestEnv::default();
    let err = eval(&expr, &env).expect_err("expected error");
    assert!(matches!(err, EvalError::EnumUnknownType { .. }));
}

#[test]
fn set_membership_true() {
    let expr = parse_expr("core.Status::Ready in { core.Status::Ready, core.Status::Pending }");
    let env = TestEnv::default().with_enum(EnumInfo {
        type_id: "core.Status".into(),
        variants: vec!["Ready".into(), "Pending".into()],
        aliases: vec![],
    });
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn set_union_cardinality() {
    let expr = parse_expr("card({1, 2} union {2, 3})");
    let env = TestEnv::default();
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Int(3));
}

#[test]
fn set_element_mismatch_errors() {
    let expr = parse_expr("{1, true}");
    let env = TestEnv::default();
    let err = eval(&expr, &env).expect_err("expected error");
    assert!(matches!(err, EvalError::SetElemMismatch { .. }));
}
