use std::collections::HashMap;

use expr::{
    eval,
    tokenize,
    Env,
    EvalError,
    LatticeOps,
    Parser,
    Value,
    TypeTag,
};

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

    fn lattice_ops(&self, type_id: &str) -> Option<LatticeOps> {
        if type_id == "core.Int" {
            Some(LatticeOps {
                type_id: type_id.into(),
                has_top: false,
                has_bottom: false,
                top: None,
                bottom: None,
                meet: |a, b| match (a, b) {
                    (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.min(y))),
                    _ => Err(EvalError::LatticeRequired { ty: TypeTag::Int, span: expr::Span::default() }),
                },
                join: |a, b| match (a, b) {
                    (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x.max(y))),
                    _ => Err(EvalError::LatticeRequired { ty: TypeTag::Int, span: expr::Span::default() }),
                },
            })
        } else {
            None
        }
    }
}

fn parse_expr(src: &str) -> expr::Expr {
    let tokens = tokenize(src).expect("tokenize");
    let mut parser = Parser::new(&tokens);
    parser.parse_expr().expect("parse")
}

#[test]
fn reduce_meet_on_ints() {
    let expr = parse_expr("reduce_meet({3, 1, 2})");
    let value = eval(&expr, &TestEnv::default()).expect("eval");
    assert_eq!(value, Value::Int(1));
}

#[test]
fn reduce_join_missing_lattice_errors() {
    let expr = parse_expr("reduce_join({ true, false })");
    let err = eval(&expr, &TestEnv::default()).expect_err("expected error");
    assert!(matches!(err, EvalError::LatticeRequired { .. }));
}
