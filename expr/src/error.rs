use crate::token::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnterminatedString { span: Span },
    UnknownChar { ch: char, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    ExpectedOperand { span: Span },
    ExpectedToken { expected: &'static str, span: Span },
    UnexpectedToken { span: Span },
    UnexpectedEof { span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeTag {
    Int,
    Bool,
    Str,
    Enum(String),
    Set(Box<TypeTag>),
    Interval(Box<TypeTag>),
    Lambda,
    Unknown,
}

impl TypeTag {
    pub fn set(inner: TypeTag) -> Self {
        TypeTag::Set(Box::new(inner))
    }

    pub fn interval(inner: TypeTag) -> Self {
        TypeTag::Interval(Box::new(inner))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    TypeMismatch {
        op: &'static str,
        left: TypeTag,
        right: TypeTag,
        span: Span,
    },
    UnknownIdent { name: String, span: Span },
    UnknownField { base: String, field: String, span: Span },
    DivideByZero { span: Span },

    EnumUnknownType { type_id: String, span: Span },
    EnumUnknownVariant { type_id: String, variant: String, span: Span },
    EqMismatch { left_ty: TypeTag, right_ty: TypeTag, span: Span },
    OrderMissing { ty: TypeTag, span: Span },
    OrderAmbiguous { ty: TypeTag, span: Span },
    OrderScopeUnknown { order_id: String, span: Span },

    InOnNonSet { rhs_ty: TypeTag, span: Span },
    SetElemMismatch { expected: TypeTag, found: TypeTag, span: Span },
    SetTypeMismatch { lhs_ty: TypeTag, rhs_ty: TypeTag, span: Span },

    IntervalTypeMismatch { lo_ty: TypeTag, hi_ty: TypeTag, x_ty: TypeTag, span: Span },
    IntervalInvalidBounds { span: Span },
    IntervalInfiniteUnsupported { ty: TypeTag, span: Span },

    CallUnknown { func: String, span: Span },
    LambdaArity { expected: usize, found: usize, span: Span },
    LambdaCaptureUnknown { name: String, span: Span },
    LambdaType { param: String, expected: TypeTag, found: TypeTag, span: Span },

    SemiringMissing { span: Span },
    LatticeRequired { ty: TypeTag, span: Span },
    CollectionUnknown { name: String, span: Span },
    QuantifierTypeMismatch { quantifier: &'static str, found: TypeTag, span: Span },

    Other { message: String, span: Span },
}
