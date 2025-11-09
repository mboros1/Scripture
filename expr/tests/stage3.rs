use std::collections::HashMap;

use expr::{eval, tokenize, Env, Parser, Value, SetValue, TypeTag};

#[derive(Default)]
struct TestEnv {
    idents: HashMap<String, Value>,
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

    fn capture_snapshot(&self) -> Vec<(String, Value)> {
        self.idents.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

fn parse_expr(src: &str) -> expr::Expr {
    let tokens = tokenize(src).expect("tokenize");
    let mut parser = Parser::new(&tokens);
    parser.parse_expr().expect("parse")
}

#[test]
fn map_transforms_set() {
    let expr = parse_expr("map({1, 2}, x -> x + 1)");
    let env = TestEnv::default();
    let value = eval(&expr, &env).expect("eval");
    let expected = Value::Set(SetValue {
        elem_type: TypeTag::Int,
        elements: vec![Value::Int(2), Value::Int(3)],
    });
    assert_eq!(value, expected);
}

#[test]
fn filter_pipeline_keeps_matching() {
    let expr = parse_expr("{1, 2, 3} |> filter(x -> x == 2)");
    let env = TestEnv::default();
    let value = eval(&expr, &env).expect("eval");
    let expected = Value::Set(SetValue {
        elem_type: TypeTag::Int,
        elements: vec![Value::Int(2)],
    });
    assert_eq!(value, expected);
}

#[test]
fn fold_pipeline_sums_values() {
    let expr = parse_expr("{1, 2, 3} |> fold(0, (acc, x) -> acc + x)");
    let env = TestEnv::default();
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Int(6));
}
