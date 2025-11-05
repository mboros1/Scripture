# RUNTIME — Call Execution Glue (v0)

**Status:** Draft (v0)  
**Purpose:** Defines the runtime components that execute `<call>` nodes after validation. Covers registries, invocation flow, interaction with the expression evaluator, and expectations for result handling.

---

## 1. Runtime Goals

- Resolve previously validated call sites into concrete function/method implementations.
- Evaluate call arguments (pure expressions) using the `expr` crate.
- Enforce purity/placement invariants at runtime as a defensive check.
- Provide a uniform logging/error surface (`EvalError`) with spans.

---

## 2. Key Types

```rust
use crate::model::{Type, Effects, Purity, Visibility, FunctionSig, MethodSig};
use crate::expr::{Expr, Value}; // Value defined below
use crate::syntax::{Call};      // from SYNTAX.md AST
```

### 2.1 Runtime Values

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
    Str(String),
    Object(String),    // opaque handle (e.g., instance identifier)
    Void,              // used for statement-position discards
}
```

- Extendable later with structs, tuples, etc.
- `Void` should never surface in value-position results; validator prevents it, runtime asserts.

### 2.2 Evaluation Context

```rust
pub struct EvalCtx<'a> {
    pub registry: &'a dyn CallRegistry,
    pub env: &'a dyn Env,     // expression environment (ident/field lookup)
}
```

- `Env` comes from the `expr` crate: resolves identifiers + field chains for expressions.
- Additional context (e.g., mutable state, heap) can be threaded as needed.

---

## 3. Call Registry

```rust
pub trait CallRegistry {
    fn lookup_free(&self, namespace: &str, name: &str, arity: usize) -> Option<&FnEntry>;
    fn lookup_method(&self, recv_ty: &Type, method: &str, arity: usize) -> Option<&FnEntry>;
}

pub struct FnEntry {
    pub sig: FnSig,
    pub imp: Box<dyn FnImpl>,
}

pub struct FnSig {
    pub namespace: Option<String>,  // None for methods
    pub name: String,               // method or function name
    pub params: Vec<Type>,
    pub ret: Type,
    pub effects: Effects,
    pub visibility: Visibility,     // methods only; Public otherwise
}

pub trait FnImpl: Send + Sync {
    fn call(&self, args: &[Value]) -> Result<Value, EvalError>;
}
```

- The registry is populated from `MODEL.md` tables; stdlib functions get concrete `FnImpl` implementations.
- `FnSig::namespace` is `None` for methods and `Some(ns)` for free functions.
- `FnImpl` returns a `Value`; statement-position calls discard the result per placement rules.

---

## 4. Evaluation Flow

```rust
pub enum CallPosition { Value, Statement }

pub fn eval_call(
    node: &Call,
    pos: CallPosition,
    ctx: &EvalCtx,
) -> Result<Value, EvalError> {
    // 1) Resolve the entry using registry
    let entry = resolve_entry(node, ctx.registry)?;

    // 2) Defensive checks (already enforced by validator)
    enforce_purity(pos, node, &entry.sig)?;
    enforce_return(pos, &entry.sig)?;

    // 3) Evaluate arguments via expr::eval
    let mut args = Vec::with_capacity(node.args.len());
    for arg_expr in &node.args {
        args.push(crate::expr::eval(arg_expr, ctx.env)?);
    }

    // 4) Type check at runtime (debug assertions / defensive)
    ensure_arg_types(&entry.sig, &args)?;

    // 5) Call implementation
    let result = entry.imp.call(&args)?;

    // 6) Handle placement
    match pos {
        CallPosition::Statement => Ok(Value::Void), // caller discards result
        CallPosition::Value => {
            if matches!(result, Value::Void) {
                return Err(EvalError::VoidInValuePosition { span: node.span });
            }
            ensure_return_type(&entry.sig.ret, &result, node)?;
            Ok(result)
        }
    }
}
```

### 4.1 Resolution Helpers

```rust
fn resolve_entry<'a>(call: &Call, registry: &'a dyn CallRegistry) -> Result<&'a FnEntry, EvalError> {
    match &call.kind {
        CallKind::Free { namespace, name } => registry
            .lookup_free(namespace, name, call.args.len())
            .ok_or_else(|| EvalError::NoSuchFunction { namespace: namespace.clone(), name: name.clone(), arity: call.args.len(), span: call.span }),
        CallKind::Method { on, method } => {
            let recv_ty = call.receiver_type
                .as_ref()
                .ok_or_else(|| EvalError::CannotInferReceiverType { on: on.clone(), span: call.span })?;
            registry
                .lookup_method(recv_ty, method, call.args.len())
                .ok_or_else(|| EvalError::NoSuchMethod { receiver: recv_ty.clone(), method: method.clone(), arity: call.args.len(), span: call.span })
        }
    }
}
```

> `Call::receiver_type` is stored by the validator/model builder during resolution so runtime does not need to recalculate it.

### 4.2 Purity and Return Enforcement

Runtime checks mirror validator expectations to guard against inconsistent registries or unchecked calls.

```rust
fn enforce_purity(pos: CallPosition, call: &Call, sig: &FnSig) -> Result<(), EvalError> {
    if matches!(pos, CallPosition::Value) && !sig.effects.is_empty() {
        return Err(EvalError::CallNotPureInValuePosition {
            callee: sig.name.clone(),
            effects: sig.effects,
            span: call.span,
        });
    }
    Ok(())
}

fn enforce_return(pos: CallPosition, sig: &FnSig) -> Result<(), EvalError> {
    if matches!(pos, CallPosition::Value) && matches!(sig.ret, Type::Void) {
        return Err(EvalError::VoidInValuePosition {
            callee: sig.name.clone(),
            span: sig.span,
        });
    }
    Ok(())
}
```

---

## 5. Error Reporting

Runtime errors reuse validator codes where possible; variants come from `EvalError`.

```rust
pub enum EvalError {
    NoSuchFunction { namespace: String, name: String, arity: usize, span: Span },
    NoSuchMethod { receiver: Type, method: String, arity: usize, span: Span },
    CannotInferReceiverType { on: IdentPath, span: Span },
    CallNotPureInValuePosition { callee: String, effects: Effects, span: Span },
    VoidInValuePosition { callee: String, span: Span },
    CallArgTypeMismatch { callee: String, index: usize, expected: Type, found: Type, span: Span },
    CallReturnTypeMismatch { callee: String, expected: Type, found: Type, span: Span },
    ImplError(String),                    // wrapper for unexpected impl failures
}
```

- `CallArgTypeMismatch` and `CallReturnTypeMismatch` catch mismatches introduced by dynamic registries or misdeclared effects.
- `ImplError` wraps implementation-defined runtime failures (`Box<dyn FnImpl>` may return domain-specific errors).

---

## 6. Integration Points

1. **Validator** resolves calls, annotates `Call` nodes with `receiver_type`, `expect`, and resolved signatures. Runtime trusts these annotations.
2. **Interpreter/Executor**:
   - `Stmt::CallStmt(call)` → `eval_call(call, CallPosition::Statement, ctx)`.
   - `ValueSource::Call(call)` → `eval_call(call, CallPosition::Value, ctx)`.
   - Other statements (`Let`, `Assign`, etc.) continue to use expression evaluator for pure expressions.
3. **Effects Accounting**:
   - After a call returns, interpreter can update state depending on `effects`.
   - For impure effects, implementation inside `FnImpl::call` performs the actual IO/mutation.

---

## 7. Registry Implementations

Common patterns:

- **Static registry:** HashMaps keyed by `(namespace, name, arity)` and `(Type, method, arity)` mapping to `FnEntry`.
- **Dynamic hooking:** runtime components may insert new entries at startup (e.g., host-provided IO functions).
- **Testing:** Provide a `MockRegistry` that records invocations and returns canned results; used in validator/runtime tests.

Example:

```rust
pub struct StaticRegistry {
    functions: HashMap<(String, String, usize), FnEntry>,
    methods: HashMap<(Type, String, usize), FnEntry>,
}

impl CallRegistry for StaticRegistry {
    fn lookup_free(&self, ns: &str, name: &str, arity: usize) -> Option<&FnEntry> {
        self.functions.get(&(ns.to_string(), name.to_string(), arity))
    }

    fn lookup_method(&self, recv_ty: &Type, method: &str, arity: usize) -> Option<&FnEntry> {
        self.methods.get(&(recv_ty.clone(), method.to_string(), arity))
    }
}
```

---

## 8. Testing Strategy

- **Unit tests**:
  - `eval_call` handles value vs statement positions.
  - Purity/return enforcement triggers expected errors.
  - Type mismatches raise `CallArgTypeMismatch`.
- **Integration tests**:
  - Run validated programs end-to-end: parse → model → validate → execute.
  - Ensure `UnusedReturnValue` lint is surfaced but does not block runtime.

---

## 9. Future Hooks

- Support asynchronous or effect-tracked execution (e.g., returning futures for IO).
- Integrate with a scheduler for deterministic effect ordering.
- Extend `Value` and `FnImpl` to support borrowing semantics or arenas once pointers/unsafe enter the language.

---

## 10. Acceptance Criteria

- `eval_call` and `CallRegistry` cover both free and method calls.
- Runtime respects validator-provided metadata (receiver types, expect).
- Defensive checks prevent purity/return violations from slipping through.
- Standard library registry entries implement `FnImpl` with correct types and effects.
