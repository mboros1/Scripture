# SYNTAX — Scripture Canonical Form (v0)

**Status:** Draft (v0)  
**Scope:** Tag-level grammar for Scripture units covering `<class>`, `<method>`, `<call>`, statements, and `<expr>` integration. This is the input that downstream passes (model tables, validator, runtime) consume.

---

## 0. Overview

- Canonical source is a restricted, XML-like syntax with required closing tags.
- Primary unit: `<class>` containing `<fields>` and `<methods>`.
- Statements support `<let>`, `<assign>`, `<return>`, `<if>`, `<while>`, and `<call>`.
- `<expr>…</expr>` bodies delegate to the expression sublanguage (pure A-expr).
- No traits/impls, imports, pointers, or overloading in v0.

---

## 1. Lexical Rules

- **Whitespace:** space, tab, carriage return, line feed — ignored between tokens.
- **Comments:** `<!-- … -->`, no nesting.
- **Identifiers (`IDENT`):** `[A-Za-z_][A-Za-z0-9_]*` (ASCII only in v0).
- **Type atoms (`TYPE`):** `i32 | bool | str | VOID | IDENT` (where `IDENT` names a class).
- **Enum atoms:**
  - Purity: `purity.PURE | purity.MUTATING | purity.IO | purity.ALLOC | purity.DEALLOC`
  - Safety: `safety.SAFE | safety.UNSAFE`
  - Visibility: `visibility.PUBLIC | visibility.PRIVATE`
- **Flags:** presence-only booleans such as `mutable`.
- **Attribute values:** atoms (unquoted). Strings appear only inside `<expr>` as part of A-expr literals.
- Unknown tags/attributes or quoted enum values (e.g., `purity="PURE"`) must be rejected.

---

## 2. File Structure (EBNF)

```
Program     := ClassDecl+

ClassDecl   := <class AttrClass> Fields? Methods? </class>

AttrClass   := name=IDENT visibilityAtom
visibilityAtom := visibility.PUBLIC | visibility.PRIVATE
```

### Fields

```
Fields      := <fields> FieldDecl+ </fields>
FieldDecl   := <field AttrField/>                // self-closing
AttrField   := name=IDENT type=TYPE (mutable)?
```

### Methods

```
Methods     := <methods> MethodDecl+ </methods>

MethodDecl  := <method AttrMethod> Params Body </method>

AttrMethod  := name=IDENT return=TYPE purityAtom safetyAtom visibilityAtom

purityAtom  := purity.PURE | purity.MUTATING | purity.IO | purity.ALLOC | purity.DEALLOC
safetyAtom  := safety.SAFE | safety.UNSAFE
```

### Parameters

```
Params      := <params> Param+ </params>
Param       := <param AttrParam/>                // self-closing
AttrParam   := name=IDENT type=TYPE (mutable)?
```

### Body

```
Body        := <body> Stmt* </body>
```

---

## 3. Statements

```
Stmt        := LetStmt
             | AssignStmt
             | ReturnStmt
             | IfStmt
             | WhileStmt
             | CallStmt
```

### Let

```
LetStmt     := <let AttrLet> ValueSource </let>
AttrLet     := name=IDENT type=TYPE

ValueSource := ExprNode | CallValue

ExprNode    := <expr> AEXPR_TEXT </expr>         // parsed via A-expr parser
```

### Assign

```
AssignStmt  := <assign AttrAssign> ExprNode </assign>
AttrAssign  := target_object=IdentPath target_field=IDENT
```

### Return

```
ReturnStmt  := <return/>
             | <return> ExprNode </return>
```

### If

```
IfStmt      := <if>
                 <cond> ExprNode </cond>
                 <then> Stmt* </then>
                 (<else> Stmt* </else>)?
               </if>
```

### While

```
WhileStmt   := <while>
                 <cond> ExprNode </cond>
                 <body> Stmt* </body>
               </while>
```

### Call

```
CallStmt    := CallNode                           // statement position
CallValue   := CallNode                           // value position

CallNode    := <call AttrCall> ArgList? </call>
             | <call AttrCall/>                   // empty-arg form

ArgList     := Arg+
Arg         := <arg> ExprNode </arg>
```

**Call attributes (mutually exclusive forms):**

```
AttrCall    := purityAtom ExpectOpt (FreeCall | MethodCall)

ExpectOpt   := expect=TYPE                      // required in value position, ignored in statement position

FreeCall    := namespace=IDENT name=IDENT
MethodCall  := method=IDENT on=IdentPath
```

**Identifier paths:**

```
IdentPath   := IDENT ('.' IDENT)*
```

---

## 4. Placement Rules (Syntax-Level)

- Value position (`<let>` initializer, `<return>` value):
  - `<call>` must include `expect=TYPE`.
  - `<expr>` must parse successfully via the A-expr parser (pure only).
- Statement position (direct child of `<body>`, `<then>`, `<else>`, `<while><body>`):
  - `<call>` may omit `expect` (ignored if provided).
- `<expr>` content is raw text delegated to the expression parser; nested `<call>` inside `<expr>` is forbidden.

---

## 5. Semantic Contracts (Validator Stage)

Enforced post-parse; listed here because grammar shapes them.

- **Resolution:**
  - Free call keys: `(namespace, name, arity)`.
  - Method call keys: `(receiver_type, method, arity)` where `receiver_type` is the static type of `on`.
- **Visibility:** `PUBLIC` vs `PRIVATE` on methods/types.
- **Purity vs placement:** Value-position calls must be `purity.PURE` and return non-`VOID`.
- **Types / arity:** Exact argument count and types; value-position `expect` must match callee return type.
- **Declared purity:** `purity.PURE` declarations must have empty effect mask.

---

## 6. Tag Tokenization Guidelines

A bespoke pull parser is sufficient; full XML support is unnecessary.

- **Open tag:** `<` IDENT (SP ATTR)* (SP)? `>`
- **Self-closing:** `<` IDENT (SP ATTR)* (SP)? `/>`
- **Close tag:** `</` IDENT (SP)? `>`
- **Attribute:** `IDENT '=' ATOM` or presence-only flag (e.g., `mutable`).
- **ATOM:** `[A-Za-z_][A-Za-z0-9_.]*` (allows dotted enums like `purity.PURE`).
- Text nodes appear only inside `<expr>` elements and should be captured verbatim.
- Parser must:
  - Reject unknown tags/attrs.
  - Reject duplicate single-use attributes.
  - Enforce exactly-one selection for enum groups (purity, safety, visibility).
  - Enforce `on` iff `method` and `expect` in value position.

---

## 7. Example (Valid)

```txt
<class name=Util visibility.PUBLIC>
  <fields></fields>

  <methods>
    <method name=length return=i32 purity.PURE safety.SAFE visibility.PUBLIC>
      <params>
        <param name=self type=Util/>
      </params>
      <body>
        <let name=h type=i32>
          <call namespace=core name=hypot expect=i32 purity.PURE>
            <arg><expr>3</expr></arg>
            <arg><expr>4</expr></arg>
          </call>
        </let>

        <call namespace=core name=log_info purity.IO>
          <arg><expr>"done"</expr></arg>
        </call>

        <return><expr>h</expr></return>
      </body>
    </method>
  </methods>
</class>
```

---

## 8. Example (Invalid)

```txt
<let name=h type=i32>
  <call namespace=core name=read_line expect=i32 purity.IO/>
</let>
```

- Parser accepts structure, but validator must emit `CallNotPureInValuePosition` because an IO call is used in value position.

---

## 9. AST Shape (Reference)

```rust
pub struct Program { pub classes: Vec<ClassDecl> }

pub struct ClassDecl {
    pub name: String,
    pub visibility: Visibility,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub span: Span,
}

pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

pub struct MethodDecl {
    pub name: String,
    pub return_ty: Type,
    pub purity: Purity,
    pub safety: Safety,
    pub visibility: Visibility,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

pub struct Block { pub stmts: Vec<Stmt>, pub span: Span }

pub enum Stmt {
    Let { name: String, ty: Type, init: ValueSource, span: Span },
    Assign { target_object: IdentPath, target_field: String, value: Expr, span: Span },
    Return { value: Option<Expr>, span: Span },
    If { cond: Expr, then_blk: Block, else_blk: Option<Block>, span: Span },
    While { cond: Expr, body: Block, span: Span },
    CallStmt(Call),
}

pub enum ValueSource { Expr(Expr), Call(Call) }

pub struct Call {
    pub kind: CallKind,
    pub expect: Option<Type>,
    pub declared_purity: Purity,
    pub args: Vec<Expr>,
    pub span: Span,
}

pub enum CallKind {
    Free   { namespace: String, name: String },
    Method { on: IdentPath, method: String },
}

pub struct IdentPath(pub Vec<String>);

// Expr is provided by the expression crate (pure A-expr).
```

---

## 10. Parser Acceptance Criteria

- Consumes any well-formed v0 program, emitting the AST above with spans.
- Enforces grammar constraints (allowed tags/attributes, mutual exclusivity, required fields).
- Captures `<expr>` bodies and converts them into `Expr` nodes using the A-expr parser.
- Produces useful errors (with spans) for structural violations.
