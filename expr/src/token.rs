use std::iter::Peekable;
use std::str::Chars;

use crate::error::LexError;

// ---------- Token & Span Definitions ----------

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // punctuation
    LParen, RParen, Dot,

    // operators
    Plus, Minus, Star, Slash, Percent,
    Lt, Le, Gt, Ge, EqEq, BangEq,
    AndAnd, OrOr,
    Bang,

    // literals
    Int(i64),
    Bool(bool),
    Str(String),
    Ident(String),

    // end of file
    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

// ---------- Tokenizer Function ----------

pub fn tokenize(src: &str) -> Result<Vec<Token>, LexError> {
    let mut chars = src.chars().peekable(); // iterator over characters
    let mut tokens = Vec::new();
    let mut index = 0; // track byte index for spans

    while let Some(&c) = chars.peek() {
        let start = index;

        let token = match c {
            '0'..='9' => read_number(&mut chars, &mut index),
            _ if is_ident_start(c) => read_ident(&mut chars, &mut index),
            '"' => read_string(&mut chars, &mut index)?,
            '+' => { chars.next(); index += 1; TokenKind::Plus },
            '-' => { chars.next(); index += 1; TokenKind::Minus },
            '*' => { chars.next(); index += 1; TokenKind::Star },
            '/' => { chars.next(); index += 1; TokenKind::Slash },
            '%' => { chars.next(); index += 1; TokenKind::Percent },
            '(' => { chars.next(); index += 1; TokenKind::LParen },
            ')' => { chars.next(); index += 1; TokenKind::RParen },
            '.' => { chars.next(); index += 1; TokenKind::Dot },
            '!' => {
                chars.next();
                index += 1;
                if let Some('=') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            '=' => {
                chars.next();
                index += 1;
                if let Some('=') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::EqEq
                } else {
                    return Err(LexError::UnknownChar {
                        ch: '=',
                        span: Span { start, end: index },
                    });
                }
            }
            '<' => {
                chars.next();
                index += 1;
                if let Some('=') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                chars.next();
                index += 1;
                if let Some('=') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                chars.next();
                index += 1;
                if let Some('&') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::AndAnd
                } else {
                    return Err(LexError::UnknownChar {
                        ch: '&',
                        span: Span { start, end: index },
                    });
                }
            }
            '|' => {
                chars.next();
                index += 1;
                if let Some('|') = chars.peek() {
                    chars.next();
                    index += 1;
                    TokenKind::OrOr
                } else {
                    return Err(LexError::UnknownChar {
                        ch: '|',
                        span: Span { start, end: index },
                    });
                }
            }
            ' ' | '\t' | '\n' | '\r' => { // skip whitespace
                chars.next();
                index += 1;
                continue;
            }
            _ => {
                return Err(LexError::UnknownChar {
                    ch: c,
                    span: Span { start, end: index },
                })
            }
        };

        tokens.push(Token { kind: token, span: Span { start, end: index } });
    }

    tokens.push(Token { kind: TokenKind::Eof, span: Span { start: index, end: index } });
    Ok(tokens)
}

// ---------- Helper Functions ----------

fn read_number(chars: &mut Peekable<Chars>, index: &mut usize) -> TokenKind {
    let mut num = 0i64;
    while let Some(&c) = chars.peek() {
        if let Some(d) = c.to_digit(10) {
            num = num * 10 + d as i64;
            chars.next();
            *index += 1;
        } else {
            break;
        }
    }
    TokenKind::Int(num)
}

fn read_ident(chars: &mut Peekable<Chars>, index: &mut usize) -> TokenKind {
    let mut name = String::new();
    while let Some(&c) = chars.peek() {
        if is_ident_continue(c) {
            name.push(c);
            chars.next();
            *index += 1;
        } else {
            break;
        }
    }
    match name.as_str() {
        "true" => TokenKind::Bool(true),
        "false" => TokenKind::Bool(false),
        _ => TokenKind::Ident(name),
    }
}

fn read_string(chars: &mut Peekable<Chars>, index: &mut usize) -> Result<TokenKind, LexError> {
    chars.next(); // skip opening quote
    *index += 1;
    let start_index = *index;
    let mut s = String::new();

    while let Some(&c) = chars.peek() {
        if c == '"' {
            chars.next();
            *index += 1;
            return Ok(TokenKind::Str(s));
        } else if c == '\n' {
            return Err(LexError::UnterminatedString {
                span: Span { start: start_index - 1, end: *index },
            });
        } else {
            s.push(c);
            chars.next();
            *index += 1;
        }
    }

    Err(LexError::UnterminatedString {
        span: Span { start: start_index - 1, end: *index },
    })
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}
