use ariadne::Source;
use bumpalo::Bump;
use crate::lexer::tokenize;
use crate::parser::MincamlParser;
use crate::diagnostics::ParserError;

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod diagnostics;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let file = args[0].clone();
    let file_id: &str = &file;
    let src  = std::fs::read_to_string(&file).unwrap();

    let tokens = tokenize(&src).unwrap_or_else(|msg| {
        eprintln!("{:?}", msg);
        vec![]
    });

    let arena = Bump::new();
    let mut parser = MincamlParser::new(tokens.into_iter(), &arena);

    match parser.pratt_parse() {
        Ok(ast) => {
            println!("{:#?}", ast);
        }
        Err(ParserError(report)) => {
            report
                .print(Source::from(src.as_str()))
                .unwrap();
        }
    }
}