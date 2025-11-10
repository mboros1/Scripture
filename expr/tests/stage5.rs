use std::collections::HashMap;

use expr::{
    eval,
    tokenize,
    Env,
    EvalError,
    Parser,
    SemiringOps,
    Span,
    TypeTag,
    Value,
};

#[derive(Default)]
struct TestEnv {
    semirings: HashMap<String, SemiringOps>,
}

impl TestEnv {
    fn with_semiring(mut self, name: &str, ops: SemiringOps) -> Self {
        self.semirings.insert(name.to_string(), ops);
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

    fn semiring(&self, name: &str) -> Option<SemiringOps> {
        self.semirings.get(name).cloned()
    }
}

fn parse_expr(src: &str) -> expr::Expr {
    let tokens = tokenize(src).expect("tokenize");
    let mut parser = Parser::new(&tokens);
    parser.parse_expr().expect("parse")
}

fn min_plus_semiring() -> SemiringOps {
    SemiringOps {
        name: "core.MinPlus".into(),
        zero: Value::Int(i64::MAX),
        one: Value::Int(0),
        oplus: |a, b| match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int((*x).min(*y))),
            _ => Err(EvalError::TypeMismatch {
                op: "oplus",
                left: TypeTag::Int,
                right: TypeTag::Int,
                span: Span::default(),
            }),
        },
        otimes: |a, b| match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(*x + *y)),
            _ => Err(EvalError::TypeMismatch {
                op: "otimes",
                left: TypeTag::Int,
                right: TypeTag::Int,
                span: Span::default(),
            }),
        },
        idempotent_oplus: true,
        commutative_otimes: true,
    }
}

#[test]
fn reduce_semiring_requires_context() {
    let expr = parse_expr("reduce_semiring({1, 2}, x -> x)");
    let err = eval(&expr, &TestEnv::default()).expect_err("expected error");
    assert!(matches!(err, EvalError::SemiringMissing { .. }));
}

#[test]
fn reduce_semiring_with_context_works() {
    let expr = parse_expr("with_semiring core.MinPlus: reduce_semiring({3, 1, 5}, x -> x)");
    let env = TestEnv::default().with_semiring("core.MinPlus", min_plus_semiring());
    let value = eval(&expr, &env).expect("eval");
    assert_eq!(value, Value::Int(1));
}
