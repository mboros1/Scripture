# CALLS — Function & Method Calls (v0)

**Status:** Draft (v0)  
**Scope:** Syntax, resolution, purity/effects, visibility, namespaces, validator rules, error codes, lints, and runtime hooks for `<call>` in Scripture.

## 1. Purpose

`<call>` is the only way to invoke behavior in Scripture.

- **Value position:** Pure computations that return a value.
- **Statement position:** Any effectful call (IO, mutation, alloc/free) and pure calls whose result is intentionally ignored (allowed with lint).

Non-goal (v0): No calls inside `<expr>`. A-expr remains pure.

## 2. Terminology

- **Free function:** Resolved by `(namespace, name, arity)`.
- **Method:** Resolved by `(receiver_type, method, arity)`.
- **Receiver type:** Static type of the `on=` target.
- **Effects:** Bitset of side effects (`W` write, `I` IO, `A` alloc, `F` free).
- **Purity class (derived):** `PURE`, `MUTATING`, `IO`, `ALLOC`, `DEALLOC`.

## 3. Canonical Syntax

### 3.1 Free function (value position; must be PURE)

```txt
<let name=hyp type=i32>
  <call namespace=core name=hypot expect=i32 purity.PURE>
    <arg><expr>a</expr></arg>
    <arg><expr>b</expr></arg>
  </call>
</let>
```

### 3.2 Free function (statement position; any effects)

```txt
<call namespace=core name=log_info purity.IO>
  <arg><expr>"swapping pairs"</expr></arg>
</call>
```

### 3.3 Method (value position; must be PURE)

```txt
<let name=len type=i32>
  <call method=length on=self expect=i32 purity.PURE/>
</let>
```

### 3.4 Method (statement position; any effects)

```txt
<call method=clear on=self purity.MUTATING/>
```

**Notes**

- `name` and `method` are mutually exclusive.
- `on=` is required when `method=` is present.
- `expect=TYPE` is required in value position; optional/ignored in statement position.
- Omit `namespace` only when project policy defaults to core.

## 4. Attributes (atoms/flags only)

### Dispatch

- `namespace=IDENT` (free functions)
- `name=IDENT` (free functions)
- `method=IDENT` + `on=IDENT|IDENT.path` (methods)

### Types & purity

- `expect=TYPE` (value position only)
- `purity.PURE | purity.MUTATING | purity.IO | purity.ALLOC | purity.DEALLOC`

### Args

- Zero or more `<arg> … </arg>` children; each contains exactly one `<expr>` (A-expr).
- No strings for enum-ish parameters. No booleans as `"true"` / `"false"`; use presence flags elsewhere.

## 5. Placement Rules

| Context           | Allowed purity            | Return requirement                  |
|-------------------|---------------------------|-------------------------------------|
| Value position    | `PURE` (effects = ∅)      | `expect` must equal `ret` ≠ `VOID`  |
| Statement position| Any (`PURE`/`MUT`/`IO`/`A`/`F`) | Return value may be ignored (lint) |

A-expr restriction: Calls are not allowed inside `<expr>`.

## 6. Resolution

### 6.1 Free functions

- Key: `(namespace, name, arity)`.
- No overloading in v0 (no same key with different param types).
- No defaults or varargs.

### 6.2 Methods

- Key: `(receiver_type, method, arity)`.
- Receiver type derives from the static type of `on=` (local symbols + method signature).
- No overloading in v0.
- Failure surfaces: `NoSuchFunction`, `NoSuchMethod`, `ArityMismatch`, `CannotInferReceiverType`.

## 7. Visibility

- Binary only (v0): `visibility.PUBLIC | visibility.PRIVATE` on methods and types.
- A caller inside class `T` may invoke `T`’s `PUBLIC` + `PRIVATE` methods.
- A caller outside `T` may invoke `PUBLIC` only.
- Private types are not visible outside their declaring unit (reject lookups).
- Failures: `MethodNotVisible`, `TypeNotVisible`.

## 8. Effects & Purity

### 8.1 Effect flags (internal)

- `W` — writes / state mutation
- `I` — IO / external world
- `A` — allocation
- `F` — deallocation

### 8.2 Derived purity (public attribute)

- `PURE` → effects = ∅
- `MUTATING` → includes `W`, excludes `I/A/F`
- `IO` → includes `I` (may include `W`)
- `ALLOC` → includes `A` only (MVP simplification)
- `DEALLOC` → includes `F` only (MVP simplification)

Invariant: Declaration’s `purity.X` must match computed effects of its body (validator enforced).

## 9. Types (v0)

- Exact type match on args/return.
- No implicit coercions.
- `VOID` return is not allowed in value position.
- Failures: `CallArgTypeMismatch`, `CallReturnTypeMismatch`, `VoidInValuePosition`.

## 10. Validator Rules (must implement)

**Structure**

- `name ⊕ method` (exactly one). `on` required iff `method`.
- `expect` required iff value position.

**Resolution**

- Resolve callee entry; else `NoSuchFunction | NoSuchMethod | CannotInferReceiverType`.

**Visibility**

- Enforce `PUBLIC / PRIVATE` per §7; else `MethodNotVisible | TypeNotVisible`.

**Placement vs purity**

- Value position: `effects == ∅` and `ret != VOID`; else `CallNotPureInValuePosition` or `VoidInValuePosition`.

**Types / arity**

- Exact arg count + types; else `ArityMismatch` or `CallArgTypeMismatch`.
- If `expect` present, equals declared return; else `CallReturnTypeMismatch`.

**Purity consistency (declaration)**

- If callee declares `purity.PURE` but computed body has effects → `DeclaredPureButHasEffects`.

**A-expr discipline**

- `<arg>` children must be `<expr>` only; no nested `<call>` inside `<expr>`; else `CallInsideExprForbidden`.

## 11. Lints (enabled by default)

- `UnusedReturnValue`: Statement-position call returns non-void and result unused.
- Suppression (future): per-function `allow_discard` flag or per-call `<discard/>` marker.
- `ImplicitCoreNamespace`: Missing `namespace` where policy requires explicit usage.
- `ShadowedMethodName`: Method and free function share name/arity in same namespace (advisory).

Lints do not block validation.

## 12. Error Codes (machine-parsable)

Emit as `CODE: message {data}` with spans.

- `NoSuchFunction(namespace, name, arity)`
- `NoSuchMethod(receiver_type, method, arity)`
- `CannotInferReceiverType(on)`
- `MethodNotVisible(receiver_type, method)`
- `TypeNotVisible(type)`
- `ArityMismatch(callee, expected, found)`
- `CallArgTypeMismatch(callee, index, expected, found)`
- `CallReturnTypeMismatch(callee, expected, found)`
- `VoidInValuePosition(callee)`
- `CallNotPureInValuePosition(callee, effects)`
- `DeclaredPureButHasEffects(callee, effects)`
- `CallInsideExprForbidden()`

Each error must carry a primary span and, when helpful, related spans (callee decl, arg expression).

## 13. Runtime Hooks (Rust)

### Registry

```rust
pub trait CallRegistry {
    fn lookup_free(&self, ns: &str, name: &str, arity: usize) -> Option<&FnEntry>;
    fn lookup_method(&self, recv_ty: &Type, method: &str, arity: usize) -> Option<&FnEntry>;
}
```

### FnEntry

```rust
pub struct FnSig {
    pub namespace: Option<String>,   // None for methods
    pub name: String,                // or method
    pub params: Vec<Type>,
    pub ret: Type,
    pub effects: Effects,            // W/I/A/F bitset
    pub visibility: Visibility,      // for methods/types
}
pub struct FnEntry { pub sig: FnSig, pub imp: Box<dyn FnImpl> }

pub trait FnImpl: Send + Sync {
    fn call(&self, args: &[Value]) -> Result<Value, EvalError>;
}
```

### Evaluation

```rust
pub enum CallPosition { Value, Statement }

pub fn eval_call(node: &CallNode, pos: CallPosition, ctx: &EvalCtx) -> Result<Value, EvalError> {
    // 1) resolve entry
    // 2) enforce placement/purity/visibility
    // 3) eval args with expr::eval()
    // 4) check arg types / arity
    // 5) call imp.call(args)
    // 6) check return type (and discard if Statement)
}
```

## 14. Worked Examples

**Valid: pure value call**

```txt
<let name=h type=i32>
  <call namespace=core name=hypot expect=i32 purity.PURE>
    <arg><expr>a</expr></arg>
    <arg><expr>b</expr></arg>
  </call>
</let>
```

**Invalid: IO in value position**

```txt
<let name=line type=i32>
  <call namespace=core name=read_line expect=i32 purity.IO/>
</let>
```

→ `CallNotPureInValuePosition("core::read_line", effects=I)`

**Valid: statement call returning non-void (lint)**

```txt
<call namespace=core name=time_now_us purity.PURE/>
```

→ Pass + `UnusedReturnValue` lint.

**Invalid: private method from outside**

```txt
<call method=rebuild_index on=db purity.MUTATING/>
```

→ `MethodNotVisible(receiver_type=Database, method=rebuild_index)`

## 15. Out-of-Scope (v0)

- Overloading, default args, varargs.
- Dynamic dispatch.
- Calls inside `<expr>`.
- Traits/impl coherence rules (tracked separately).
- Pointers/unsafe effects (`safety.UNSAFE`) — separate spec.

## 16. Acceptance Criteria (for this feature)

- Validator implements §10 with the error codes in §12.
- Runtime registry + evaluator implement §13.
- Golden tests: resolution (free/method), visibility, placement vs purity, type/arity.
- Lints fire for unused return values.
- Docs: this `CALLS.md` linked from `README.md`.

## 17. Future Flags (not for v0)

- `must_use` on functions → escalate lint to error when discarded.
- Import sugar: `<use namespace=core as=c/>` then `<call name=hypot namespace=c …/>`.
- Namespaced methods (if/when traits land).
- Effect subsets (e.g., “logical purity” allowing alloc) — only if we can preserve deterministic guarantees.

## 18. Design Stance (frozen for v0)

- Statement non-void → allow + lint.
- `namespace` → required attr, default core.
- Visibility → `PUBLIC` / `PRIVATE` only, method calls enforced via receiver type.
- `purity.PURE` → effects = ∅. No exceptions.
