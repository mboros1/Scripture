// expr/src/parser.rs
use crate::token::{Token, TokenKind};
use crate::ast::{Expr, BOp, UOp, IdentPath};
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
        use TokenKind::*;

        // ---- prefix / primary ----
        let mut lhs = match self.next_token().ok_or(ParseError::UnexpectedEOF)? {
            Token { kind: TokenKind::Int(n), .. } => Expr::Int(*n),

            Token { kind: TokenKind::Bool(b), .. } => Expr::Bool(*b),

            Token { kind: TokenKind::Str(s), .. } => Expr::Str(s.clone()),

            Token { kind: TokenKind::Ident(name), .. } => {
                Expr::Ident(IdentPath(vec![name.clone()]))
            }

            Token { kind: TokenKind::Bang, .. } => {
                // unary not has high precedence
                let rhs = self.parse_precedence(15)?;
                Expr::Unary { op: UOp::Not, expr: Box::new(rhs) }
            }

            Token { kind: TokenKind::Minus, .. } => {
                let rhs = self.parse_precedence(15)?;
                Expr::Unary { op: UOp::Neg, expr: Box::new(rhs) }
            }

            Token { kind: TokenKind::LParen, .. } => {
                let expr = self.parse_expr()?;
                // expect RParen
                match self.next_token() {
                    Some(Token { kind: TokenKind::RParen, .. }) => (),
                    _ => return Err(ParseError::UnexpectedToken("expected ')'".into())),
                }
                expr
            }

            other => {
                return Err(ParseError::UnexpectedToken(format!("unexpected token: {:?}", other)));
            }
        };

        // ---- infix loop ----
        loop {
            // peek next token kind
            let next_kind = match self.peek_token() {
                Some(t) => &t.kind,
                None => break,
            };

            // stop on ) or EOF
            if matches!(next_kind, TokenKind::RParen | TokenKind::Eof) {
                break;
            }

            // get precedence info
            let (l_bp, r_bp, bop) = match get_precedence(next_kind) {
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

            lhs = Expr::Binary { left: Box::new(lhs), op: bop, right: Box::new(rhs) };
        }

        Ok(lhs)
    }
}

/// Map TokenKind -> (left_bp, right_bp, BOp)
fn get_precedence(tok: &TokenKind) -> Option<(u8, u8, BOp)> {
    use TokenKind::*;

    match tok {
        // * / %
        Star => Some((11, 12, BOp::Mul)),
        Slash => Some((11, 12, BOp::Div)),
        Percent => Some((11, 12, BOp::Rem)),

        // + -
        Plus => Some((9, 10, BOp::Add)),
        Minus => Some((9, 10, BOp::Sub)),

        // comparisons
        Lt => Some((7, 8, BOp::Lt)),
        Le => Some((7, 8, BOp::Le)),
        Gt => Some((7, 8, BOp::Gt)),
        Ge => Some((7, 8, BOp::Ge)),

        // equality
        EqEq => Some((5, 6, BOp::Eq)),
        BangEq => Some((5, 6, BOp::Ne)),

        // logical
        AndAnd => Some((3, 4, BOp::And)),
        OrOr => Some((1, 2, BOp::Or)),

        _ => None,
    }
}
