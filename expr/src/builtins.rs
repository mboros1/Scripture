use crate::error::{EvalError, TypeTag};
use crate::runtime::Env;
use crate::token::Span;
use crate::value::{contains_value, LatticeOps, LambdaValue, SetValue, Value};

type LambdaCaller<'a> = &'a mut dyn FnMut(&LambdaValue, &[Value], &dyn Env) -> Result<Value, EvalError>;

pub fn apply(
    name: &str,
    args: Vec<Value>,
    env: &dyn Env,
    mut caller: LambdaCaller,
) -> Result<Value, EvalError> {
    match name {
        "map" => builtin_map(args, env, &mut caller),
        "filter" => builtin_filter(args, env, &mut caller),
        "fold" => builtin_fold(args, env, &mut caller),
        "reduce_meet" => builtin_reduce_meet(args, env),
        "reduce_join" => builtin_reduce_join(args, env),
        "reduce_semiring" => builtin_reduce_semiring(args, env, &mut caller),
        _ => Err(EvalError::CallUnknown { func: name.into(), span: Span::default() }),
    }
}

fn builtin_map(
    args: Vec<Value>,
    env: &dyn Env,
    caller: LambdaCaller,
) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = expect_set(&args[0])?;
    let lambda = expect_lambda(&args[1], "map")?;

    let mut results = Vec::new();
    let mut elem_type = TypeTag::Unknown;
    for elem in &set.elements {
        let value = caller(&lambda, &[elem.clone()], env)?;
        if elem_type == TypeTag::Unknown {
            elem_type = value.type_tag();
        }
        if !contains_value(&results, &value)? {
            results.push(value);
        }
    }
    Ok(Value::Set(SetValue { elem_type, elements: results }))
}

fn builtin_filter(
    args: Vec<Value>,
    env: &dyn Env,
    caller: LambdaCaller,
) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = expect_set(&args[0])?;
    let lambda = expect_lambda(&args[1], "filter")?;

    let mut results = Vec::new();
    for elem in &set.elements {
        let value = caller(&lambda, &[elem.clone()], env)?;
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

fn builtin_fold(
    args: Vec<Value>,
    env: &dyn Env,
    caller: LambdaCaller,
) -> Result<Value, EvalError> {
    if args.len() != 3 {
        return Err(EvalError::LambdaArity { expected: 3, found: args.len(), span: Span::default() });
    }
    let set = expect_set(&args[0])?;
    let mut acc = args[1].clone();
    let lambda = expect_lambda(&args[2], "fold")?;

    for elem in &set.elements {
        acc = caller(&lambda, &[acc, elem.clone()], env)?;
    }
    Ok(acc)
}

fn builtin_reduce_meet(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    let set = expect_single_set(args, "reduce_meet")?;
    let lattice = expect_lattice(&set, env)?;
    fold_lattice(set, lattice.meet)
}

fn builtin_reduce_join(args: Vec<Value>, env: &dyn Env) -> Result<Value, EvalError> {
    let set = expect_single_set(args, "reduce_join")?;
    let lattice = expect_lattice(&set, env)?;
    fold_lattice(set, lattice.join)
}

fn builtin_reduce_semiring(
    args: Vec<Value>,
    env: &dyn Env,
    caller: LambdaCaller,
) -> Result<Value, EvalError> {
    if args.len() != 2 {
        return Err(EvalError::LambdaArity { expected: 2, found: args.len(), span: Span::default() });
    }
    let set = expect_set(&args[0])?;
    let lambda = expect_lambda(&args[1], "reduce_semiring")?;
    let semiring = env.current_semiring().ok_or(EvalError::SemiringMissing { span: Span::default() })?;

    let mut acc = semiring.zero.clone();
    for elem in &set.elements {
        let mapped = caller(&lambda, &[elem.clone()], env)?;
        acc = (semiring.oplus)(&acc, &mapped)?;
    }
    Ok(acc)
}

fn expect_set(value: &Value) -> Result<SetValue, EvalError> {
    match value {
        Value::Set(set) => Ok(set.clone()),
        other => Err(EvalError::InOnNonSet { rhs_ty: other.type_tag(), span: Span::default() }),
    }
}

fn expect_single_set(args: Vec<Value>, _name: &str) -> Result<SetValue, EvalError> {
    if args.len() != 1 {
        return Err(EvalError::LambdaArity { expected: 1, found: args.len(), span: Span::default() });
    }
    expect_set(&args[0])
}

fn expect_lambda(value: &Value, label: &str) -> Result<LambdaValue, EvalError> {
    match value {
        Value::Lambda(lambda) => Ok(lambda.clone()),
        other => Err(EvalError::LambdaType {
            param: label.into(),
            expected: TypeTag::Lambda,
            found: other.type_tag(),
            span: Span::default(),
        }),
    }
}

fn expect_lattice(set: &SetValue, env: &dyn Env) -> Result<LatticeOps, EvalError> {
    let ty_id = match &set.elem_type {
        TypeTag::Enum(name) => name.clone(),
        TypeTag::Int => "core.Int".into(),
        other => format!("{:?}", other),
    };
    env.lattice_ops(&ty_id)
        .ok_or(EvalError::LatticeRequired { ty: set.elem_type.clone(), span: Span::default() })
}

fn fold_lattice(
    set: SetValue,
    op: fn(&Value, &Value) -> Result<Value, EvalError>,
) -> Result<Value, EvalError> {
    let mut iter = set.elements.iter();
    let mut acc = iter
        .next()
        .cloned()
        .ok_or(EvalError::Other { message: "lattice reduction on empty set".into(), span: Span::default() })?;
    for elem in iter {
        acc = op(&acc, elem)?;
    }
    Ok(acc)
}
