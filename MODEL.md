# MODEL — Types, Symbols, and Effects (v0)

**Status:** Draft (v0)  
**Purpose:** Defines the shared data model that bridges parsed Scripture syntax (see `SYNTAX.md`) to the validator and runtime layers. Covers type representations, symbol tables, purity/effects metadata, and standard library seeding.

---

## 1. Goals

- Provide canonical representations for types, purity, effects, and visibility.
- Maintain lookup tables for classes, methods, and free functions.
- Record declaration metadata (spans, attributes) needed by validator and runtime.
- Surface effect masks for purity checking and runtime enforcement.

---

## 2. Core Types

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    Bool,
    Str,
    Void,
    Named(String), // user-defined class or alias
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purity {
    Pure,
    Mutating,
    Io,
    Alloc,
    Dealloc,
}

bitflags::bitflags! {
    pub struct Effects: u8 {
        const W = 0b0001; // write / mutation
        const I = 0b0010; // IO / external
        const A = 0b0100; // allocation
        const F = 0b1000; // deallocation
    }
}
```

**Purity ↔ Effects mapping (validator enforced):**

| Purity        | Legal effect mask |
|---------------|-------------------|
| `Pure`        | `Effects::empty()` |
| `Mutating`    | `Effects::W` (no `I/A/F`) |
| `Io`          | must include `Effects::I` (may also include `W`) |
| `Alloc`       | exactly `Effects::A` |
| `Dealloc`     | exactly `Effects::F` |

---

## 3. Symbol Tables

Layered lookups tying program declarations to metadata. All entries carry source spans for error reporting.

### 3.1 Type Table

```rust
pub struct TypeEntry {
    pub name: String,
    pub visibility: Visibility,
    pub span: Span,            // span of the <class> declaration
}

pub struct TypeTable {
    pub classes: HashMap<String, TypeEntry>,
}
```

- Populated from `ClassDecl` nodes.
- Validator uses it to confirm type visibility for method calls and to resolve receiver types.

### 3.2 Method Table

```rust
pub struct MethodSig {
    pub receiver: Type,            // Named(class_name)
    pub name: String,
    pub params: Vec<Type>,
    pub ret: Type,
    pub declared_purity: Purity,
    pub effects: Effects,          // computed from body
    pub visibility: Visibility,
    pub span: Span,                // <method>
}

pub struct MethodTable {
    // Keyed by (receiver_type, method_name, arity)
    pub methods: HashMap<(String, String, usize), MethodSig>,
}
```

- `effects` is derived post-parse by scanning method bodies.
- `declared_purity` reflects the `purity.*` attribute; validator compares the two.
- Additional metadata (e.g., safety) can be added later.

### 3.3 Function Table

```rust
pub struct FunctionSig {
    pub namespace: String,
    pub name: String,
    pub params: Vec<Type>,
    pub ret: Type,
    pub declared_purity: Purity,
    pub effects: Effects,      // from declaration or stdlib seed
    pub span: Span,            // <function> (future) or synthetic for stdlib
}

pub struct FunctionTable {
    // Keyed by (namespace, name, arity)
    pub functions: HashMap<(String, String, usize), FunctionSig>,
}
```

- v0 programs only consume stdlib free functions (core namespace). When free function declarations land, they will populate this table directly.

---

## 4. Environment During Validation

### 4.1 Symbol Environment (per method)

The validator maintains an environment describing locals, parameters, and their types:

```rust
pub struct SymbolEnv<'a> {
    pub self_type: Option<Type>,             // class for methods
    pub locals: HashMap<String, Type>,
    pub params: HashMap<String, Type>,
    pub method_table: &'a MethodTable,
    pub function_table: &'a FunctionTable,
    pub type_table: &'a TypeTable,
}
```

- `locals` are filled as `<let>` bindings appear.
- `SymbolEnv::lookup_ident` resolves order: locals → params → `self` fields (via `TypeTable` + class fields).

### 4.2 Field Metadata

Maintain a mapping of class fields for type lookup and mutability checks:

```rust
pub struct FieldEntry {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

pub type FieldTable = HashMap<String, Vec<FieldEntry>>; // key: class name
```

---

## 5. Effects Computation

Computed per method to verify declared purity and to supply runtime metadata.

### 5.1 Traversal Rules

- `Stmt::Assign` → add `Effects::W`.
- `Stmt::CallStmt(call)` → union `effects_of(call)`.
- `ValueSource::Call(call)` (in value position) → union `effects_of(call)` even though validator should enforce PURE.
- Control flow (`If`, `While`) recursively union effects.
- Additional statements (future) should specify their effect contributions.

### 5.2 Call Effects

```
fn effects_of(call: &Call, tables: &Tables) -> Effects {
    match call.kind {
        CallKind::Free { namespace, name } => tables.function.lookup(namespace, name, arity).effects,
        CallKind::Method { on_type, method } => tables.method.lookup(on_type, method, arity).effects,
    }
}
```

- `arity = call.args.len()`.
- Missing entries are caught earlier by resolution; this helper assumes successful lookup.

---

## 6. Type Checking Helpers

Tied closely to validator but specified here so both validator and runtime can share logic.

```rust
pub enum TypeErrorKind {
    UnknownIdent { name: String },
    UnknownField { base: Type, field: String },
    Mismatch { expected: Type, found: Type },
    VoidInValuePosition,
}

pub struct TypeError {
    pub kind: TypeErrorKind,
    pub span: Span,
}

pub fn type_of_expr(expr: &Expr, env: &SymbolEnv, fields: &FieldTable) -> Result<Type, TypeError>;
```

- Handles literals, identifiers, and dotted field access.
- `Expr::Call` is not possible (enforced by syntax).
- Used for:
  - Checking `<let>` initializers.
  - Validating call argument types.
  - Ensuring `expect` matches callee return type.

---

## 7. Standard Library Seeding

Populate tables with core functions and effect metadata. Example:

```rust
pub fn seed_stdlib(functions: &mut FunctionTable, methods: &mut MethodTable) {
    functions.insert(FunctionSig {
        namespace: "core".into(),
        name: "hypot".into(),
        params: vec![Type::I32, Type::I32],
        ret: Type::I32,
        declared_purity: Purity::Pure,
        effects: Effects::empty(),
        span: Span::stdlib("core::hypot"),
    });

    functions.insert(FunctionSig {
        namespace: "core".into(),
        name: "log_info".into(),
        params: vec![Type::Str],
        ret: Type::Void,
        declared_purity: Purity::Io,
        effects: Effects::I,
        span: Span::stdlib("core::log_info"),
    });

    functions.insert(FunctionSig {
        namespace: "core".into(),
        name: "time_now_us".into(),
        params: vec![],
        ret: Type::I32,
        declared_purity: Purity::Pure,
        effects: Effects::empty(),
        span: Span::stdlib("core::time_now_us"),
    });
}
```

- `Span::stdlib` can be a synthetic helper for diagnostics (e.g., `Span { start: 0, end: 0, origin: Std("core::hypot") }`).
- Method seeds follow the same pattern when built-in classes arrive.

---

## 8. Data Flow Summary

1. **Syntax parser** (`SYNTAX.md`) produces `Program` AST with spans.
2. **Model builder**:
   - Populates `TypeTable`, `FieldTable`, `MethodTable`, `FunctionTable`.
   - Computes `Effects` for each method.
3. **Validator** consumes the model to:
   - Resolve call targets.
   - Enforce purity/visibility/type rules.
   - Emit structured diagnostics and lints.
4. **Runtime** uses the same tables to wire `CallRegistry` and `eval_call`.

---

## 9. Acceptance Criteria

- Every declared class, method, and field produces table entries with correct spans.
- Effect computation is stable (deterministic, memoized per method).
- Tables reject duplicate keys (same `(namespace, name, arity)` or `(receiver, method, arity)`).
- Seeded stdlib entries provide return types, purity, and effects consistent with CALLS validator expectations.
