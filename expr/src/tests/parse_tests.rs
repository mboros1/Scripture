mod token;
mod parser;
mod ast;

use parser::Parser;
use token::tokenize;

fn main() {
    let source = "1 + 2 * 3";
    let tokens = tokenize(source).unwrap();
    println!("Tokens: {:#?}", tokens);

    let mut parser = Parser::new(&tokens);
    let expr = parser.parse_expr().unwrap();
    println!("Parsed Expression: {:#?}", expr);
}