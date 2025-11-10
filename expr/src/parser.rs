// expr/src/parser.rs
use crate::token::{Span, Token, TokenKind};
use crate::ast::{
    Expr, BOp, UOp, IdentPath, SetOp, IntervalExpr, BoundExpr, InfinityKind, Call,
};
use crate::error::ParseError;

/// Simple parser over a slice of Tokens
pub struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn next_token(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn peek_token(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_precedence(0)
    }

    fn parse_precedence(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        // ---- prefix / primary ----
        let token = self
            .next_token()
            .ok_or(ParseError::UnexpectedEof { span: self.eof_span() })?;

        let mut lhs = match &token.kind {
            TokenKind::Int(n) => Expr::Int(*n),

            TokenKind::Bool(b) => Expr::Bool(*b),

            TokenKind::Str(s) => Expr::Str(s.clone()),

            TokenKind::Ident(name) => self.parse_ident_or_lambda(name.clone())?,

            TokenKind::Bang => {
                let rhs = self.parse_precedence(15)?;
                Expr::Unary { op: UOp::Not, expr: Box::new(rhs) }
            }

            TokenKind::Minus => {
                let rhs = self.parse_precedence(15)?;
                Expr::Unary { op: UOp::Neg, expr: Box::new(rhs) }
            }

            TokenKind::Card => {
                let inner = self.parse_parenthesized_expr()?;
                Expr::Cardinality(Box::new(inner))
            }

            TokenKind::PlusInf => Expr::Infinity(InfinityKind::Positive),
            TokenKind::MinusInf => Expr::Infinity(InfinityKind::Negative),

            TokenKind::LParen => {
                if let Some(lambda) = self.try_parse_lambda_from_paren()? {
                    lambda
                } else if let Some(interval) = self.try_parse_interval_from_paren()? {
                    interval
                } else {
                    let expr = self.parse_expr()?;
                    match self.next_token() {
                        Some(Token { kind: TokenKind::RParen, .. }) => expr,
                        Some(tok) => {
                            return Err(ParseError::ExpectedToken {
                                expected: ")",
                                span: tok.span,
                            })
                        }
                        None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
                    }
                }
            }

            TokenKind::LBracket => self.parse_interval_literal(BoundKind::Closed)?,

            TokenKind::LBrace => self.parse_set_literal()?,

            TokenKind::WithSemiring => self.parse_with_semiring_expr()?,

            TokenKind::Pipe => {
                return Err(ParseError::UnexpectedToken { span: token.span });
            }

            _ => {
                return Err(ParseError::UnexpectedToken { span: token.span });
            }
        };

        // ---- infix loop ----
        loop {
            // peek next token kind
            let next_kind = match self.peek_token() {
                Some(t) => &t.kind,
                None => break,
            };

            // stop on closing delimiters or EOF
            if matches!(
                next_kind,
                TokenKind::RParen
                    | TokenKind::RBrace
                    | TokenKind::RBracket
                    | TokenKind::Eof
                    | TokenKind::Comma
                    | TokenKind::Pipe
            ) {
                break;
            }

            // get precedence info
            let (l_bp, r_bp, op) = match get_precedence(next_kind) {
                Some(p) => p,
                None => break,
            };

            if l_bp < min_bp {
                break;
            }

            // consume operator token
            self.next_token();

            // parse rhs with right binding power
            let rhs = self.parse_precedence(r_bp)?;

            lhs = match op {
                InfixOp::Binary(bop) => Expr::Binary {
                    left: Box::new(lhs),
                    op: bop,
                    right: Box::new(rhs),
                },
                InfixOp::Set(op) => Expr::SetOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                InfixOp::In => Expr::In {
                    elem: Box::new(lhs),
                    set: Box::new(rhs),
                },
            };
        }

        while matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Pipe)) {
            self.next_token();
            let call = self.parse_call_expr()?;
            lhs = Expr::Pipe { lhs: Box::new(lhs), call };
        }

        Ok(lhs)
    }
}

impl<'a> Parser<'a> {
    fn eof_span(&self) -> Span {
        self.tokens.last().map(|t| t.span).unwrap_or_default()
    }

    fn parse_ident_or_lambda(&mut self, first: String) -> Result<Expr, ParseError> {
        if matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Arrow)) {
            self.next_token();
            let body = self.parse_expr()?;
            return Ok(Expr::Lambda { params: vec![first], body: Box::new(body) });
        }
        self.parse_ident_like(first)
    }

    fn parse_ident_like(&mut self, first: String) -> Result<Expr, ParseError> {
        let mut parts = vec![first];
        while matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Dot)) {
            self.next_token(); // consume dot
            match self.next_token() {
                Some(Token { kind: TokenKind::Ident(name), .. }) => parts.push(name.clone()),
                Some(tok) => {
                    return Err(ParseError::ExpectedToken {
                        expected: "identifier",
                        span: tok.span,
                    })
                }
                None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
            }
        }

        if matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::ColonColon)) {
            self.next_token(); // consume ::
            let variant_tok = self
                .next_token()
                .ok_or(ParseError::ExpectedToken { expected: "variant", span: self.eof_span() })?;
            match &variant_tok.kind {
                TokenKind::Ident(variant) => {
                    let ty = parts.join(".");
                    Ok(Expr::EnumVariant { ty, variant: variant.clone() })
                }
                _ => Err(ParseError::ExpectedToken { expected: "variant", span: variant_tok.span }),
            }
        } else if matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::LParen)) {
            let call = self.parse_call_with_name(parts)?;
            Ok(Expr::Call(call))
        } else {
            Ok(Expr::Ident(IdentPath(parts)))
        }
    }

    fn parse_parenthesized_expr(&mut self) -> Result<Expr, ParseError> {
        match self.next_token() {
            Some(Token { kind: TokenKind::LParen, .. }) => {}
            Some(tok) => {
                return Err(ParseError::ExpectedToken {
                    expected: "(",
                    span: tok.span,
                })
            }
            None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
        }
        let expr = self.parse_expr()?;
        match self.next_token() {
            Some(Token { kind: TokenKind::RParen, .. }) => Ok(expr),
            Some(tok) => Err(ParseError::ExpectedToken {
                expected: ")",
                span: tok.span,
            }),
            None => Err(ParseError::UnexpectedEof { span: self.eof_span() }),
        }
    }

    fn parse_set_literal(&mut self) -> Result<Expr, ParseError> {
        let mut elements = Vec::new();

        loop {
            match self.peek_token() {
                Some(Token { kind: TokenKind::RBrace, .. }) => {
                    self.next_token();
                    break;
                }
                Some(_) => {
                    let expr = self.parse_precedence(0)?;
                    elements.push(expr);
                    match self.peek_token() {
                        Some(Token { kind: TokenKind::Comma, .. }) => {
                            self.next_token();
                        }
                        Some(Token { kind: TokenKind::RBrace, .. }) => {
                            self.next_token();
                            break;
                        }
                        Some(tok) => {
                            return Err(ParseError::ExpectedToken {
                                expected: "comma or '}'",
                                span: tok.span,
                            })
                        }
                        None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
                    }
                }
                None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
            }
        }

        Ok(Expr::SetLiteral(elements))
    }

    fn try_parse_interval_from_paren(&mut self) -> Result<Option<Expr>, ParseError> {
        let start_pos = self.pos;
        let lower_expr = match self.parse_interval_endpoint_expr() {
            Ok(expr) => expr,
            Err(err) => {
                self.pos = start_pos;
                return Err(err);
            }
        };

        if !matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Comma)) {
            self.pos = start_pos;
            return Ok(None);
        }
        self.next_token(); // consume comma

        match self.parse_interval_after_lower(BoundKind::Open, lower_expr) {
            Ok(expr) => Ok(Some(expr)),
            Err(err) => {
                self.pos = start_pos;
                Err(err)
            }
        }
    }

    fn parse_with_semiring_expr(&mut self) -> Result<Expr, ParseError> {
        let name = self.parse_dotted_ident_string()?;
        match self.next_token() {
            Some(Token { kind: TokenKind::Colon, .. }) => {}
            Some(tok) => {
                return Err(ParseError::ExpectedToken {
                    expected: ":",
                    span: tok.span,
                })
            }
            None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
        }
        let body = self.parse_expr()?;
        Ok(Expr::WithSemiring { name, body: Box::new(body) })
    }

    fn parse_dotted_ident_string(&mut self) -> Result<String, ParseError> {
        let first = self
            .next_token()
            .ok_or(ParseError::UnexpectedEof { span: self.eof_span() })?;
        let mut parts = match &first.kind {
            TokenKind::Ident(name) => vec![name.clone()],
            _ => {
                return Err(ParseError::ExpectedToken {
                    expected: "identifier",
                    span: first.span,
                })
            }
        };
        while matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Dot)) {
            self.next_token();
            let next = self
                .next_token()
                .ok_or(ParseError::UnexpectedEof { span: self.eof_span() })?;
            match &next.kind {
                TokenKind::Ident(name) => parts.push(name.clone()),
                _ => {
                    return Err(ParseError::ExpectedToken {
                        expected: "identifier",
                        span: next.span,
                    })
                }
            }
        }
        Ok(parts.join("."))
    }

    fn parse_interval_literal(&mut self, lower_kind: BoundKind) -> Result<Expr, ParseError> {
        let lower_expr = self.parse_interval_endpoint_expr()?;
        match self.next_token() {
            Some(Token { kind: TokenKind::Comma, .. }) => (),
            Some(tok) => {
                return Err(ParseError::ExpectedToken {
                    expected: ",",
                    span: tok.span,
                })
            }
            None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
        }
        self.parse_interval_after_lower(lower_kind, lower_expr)
    }

    fn parse_interval_after_lower(
        &mut self,
        lower_kind: BoundKind,
        lower_expr: Expr,
    ) -> Result<Expr, ParseError> {
        let upper_expr = self.parse_interval_endpoint_expr()?;
        let closing = self
            .next_token()
            .ok_or(ParseError::UnexpectedEof { span: self.eof_span() })?;
        let upper_kind = match closing.kind {
            TokenKind::RBracket => BoundKind::Closed,
            TokenKind::RParen => BoundKind::Open,
            _ => {
                return Err(ParseError::ExpectedToken {
                    expected: ") or ]",
                    span: closing.span,
                })
            }
        };

        Ok(Expr::Interval(IntervalExpr {
            lo: build_bound(lower_kind, lower_expr),
            hi: build_bound(upper_kind, upper_expr),
        }))
    }

    fn parse_interval_endpoint_expr(&mut self) -> Result<Expr, ParseError> {
        match self.peek_token().map(|t| &t.kind) {
            Some(TokenKind::PlusInf) => {
                self.next_token();
                Ok(Expr::Infinity(InfinityKind::Positive))
            }
            Some(TokenKind::MinusInf) => {
                self.next_token();
                Ok(Expr::Infinity(InfinityKind::Negative))
            }
            _ => self.parse_precedence(0),
        }
    }

    fn try_parse_lambda_from_paren(&mut self) -> Result<Option<Expr>, ParseError> {
        let start_pos = self.pos;
        let mut params = Vec::new();

        if matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::RParen)) {
            self.next_token();
        } else {
            loop {
                let ident = match self.next_token() {
                    Some(Token { kind: TokenKind::Ident(name), .. }) => name.clone(),
                    _ => {
                        self.pos = start_pos;
                        return Ok(None);
                    }
                };
                params.push(ident);
                match self.peek_token() {
                    Some(Token { kind: TokenKind::Comma, .. }) => {
                        self.next_token();
                    }
                    Some(Token { kind: TokenKind::RParen, .. }) => {
                        self.next_token();
                        break;
                    }
                    _ => {
                        self.pos = start_pos;
                        return Ok(None);
                    }
                }
            }
        }

        if !matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::Arrow)) {
            self.pos = start_pos;
            return Ok(None);
        }
        self.next_token(); // consume arrow
        let body = self.parse_expr()?;
        Ok(Some(Expr::Lambda { params, body: Box::new(body) }))
    }

    fn parse_call_expr(&mut self) -> Result<Call, ParseError> {
        let func_token = self
            .next_token()
            .ok_or(ParseError::UnexpectedEof { span: self.eof_span() })?;
        match &func_token.kind {
            TokenKind::Ident(name) => self.parse_call_with_name(vec![name.clone()]),
            _ => Err(ParseError::ExpectedToken { expected: "identifier", span: func_token.span }),
        }
    }

    fn parse_call_with_name(&mut self, name_parts: Vec<String>) -> Result<Call, ParseError> {
        match self.next_token() {
            Some(Token { kind: TokenKind::LParen, .. }) => {}
            Some(tok) => {
                return Err(ParseError::ExpectedToken {
                    expected: "(",
                    span: tok.span,
                })
            }
            None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
        }

        let mut args = Vec::new();
        loop {
            if matches!(self.peek_token().map(|t| &t.kind), Some(TokenKind::RParen)) {
                self.next_token();
                break;
            }
            let expr = self.parse_expr()?;
            args.push(expr);
            match self.peek_token() {
                Some(Token { kind: TokenKind::Comma, .. }) => {
                    self.next_token();
                }
                Some(Token { kind: TokenKind::RParen, .. }) => {
                    self.next_token();
                    break;
                }
                Some(tok) => {
                    return Err(ParseError::ExpectedToken {
                        expected: ", or )",
                        span: tok.span,
                    })
                }
                None => return Err(ParseError::UnexpectedEof { span: self.eof_span() }),
            }
        }

        Ok(Call { func: name_parts.join("."), args })
    }
}

/// Map TokenKind -> (left_bp, right_bp, BOp)
fn get_precedence(tok: &TokenKind) -> Option<(u8, u8, InfixOp)> {
    use TokenKind::*;

    match tok {
        // * / %
        Star => Some((11, 12, InfixOp::Binary(BOp::Mul))),
        Slash => Some((11, 12, InfixOp::Binary(BOp::Div))),
        Percent => Some((11, 12, InfixOp::Binary(BOp::Rem))),

        // + -
        Plus => Some((9, 10, InfixOp::Binary(BOp::Add))),
        Minus => Some((9, 10, InfixOp::Binary(BOp::Sub))),

        // comparisons
        Lt => Some((7, 8, InfixOp::Binary(BOp::Lt))),
        Le => Some((7, 8, InfixOp::Binary(BOp::Le))),
        Gt => Some((7, 8, InfixOp::Binary(BOp::Gt))),
        Ge => Some((7, 8, InfixOp::Binary(BOp::Ge))),
        In => Some((7, 8, InfixOp::In)),

        // equality
        EqEq => Some((5, 6, InfixOp::Binary(BOp::Eq))),
        BangEq => Some((5, 6, InfixOp::Binary(BOp::Ne))),

        Union => Some((4, 5, InfixOp::Set(SetOp::Union))),
        Intersect => Some((4, 5, InfixOp::Set(SetOp::Intersect))),
        Diff => Some((4, 5, InfixOp::Set(SetOp::Diff))),

        // logical
        AndAnd => Some((3, 4, InfixOp::Binary(BOp::And))),
        OrOr => Some((1, 2, InfixOp::Binary(BOp::Or))),

        _ => None,
    }
}

enum InfixOp {
    Binary(BOp),
    Set(SetOp),
    In,
}

#[derive(Copy, Clone)]
enum BoundKind {
    Open,
    Closed,
}

fn build_bound(kind: BoundKind, expr: Expr) -> BoundExpr {
    match expr {
        Expr::Infinity(InfinityKind::Negative) => BoundExpr::NegInf,
        Expr::Infinity(InfinityKind::Positive) => BoundExpr::PosInf,
        other => match kind {
            BoundKind::Open => BoundExpr::Open(Box::new(other)),
            BoundKind::Closed => BoundExpr::Closed(Box::new(other)),
        },
    }
}
