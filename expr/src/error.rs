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

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    TypeMismatch {
        op: &'static str,
        left: &'static str,
        right: &'static str,
        span: Span,
    },
    UnknownIdent { name: String, span: Span },
    UnknownField { base: String, field: String, span: Span },
    DivideByZero { span: Span },
    Other { message: String, span: Span },
}
