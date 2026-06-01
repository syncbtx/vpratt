use logos::{Logos, Span};

#[derive(Logos, Debug, Copy, Clone, PartialEq)]
#[logos(skip(r"[ \t\n\r\f]+"))]
#[logos(skip(r"--[^\n]*", allow_greedy = true))]
pub enum TokenKind<'a>{
    #[token("let")]        Let,
    #[token("rec")]        Recursive,
    #[token("in")]         In,
    #[token("true")]       True,
    #[token("false")]      False,
    #[token("if")]         If,
    #[token("then")]       Then,
    #[token("else")]       Else,
    #[token("not")]        Not,

    #[token("Array.create")]
    ArrayCreate,

    #[token("+")]          Plus,
    #[token("-")]          Minus,
    #[token("*")]          Star,
    #[token("/")]          Slash,
    #[token("+.")]         PlusDot,
    #[token("-.")]         MinusDot,
    #[token("/.")]         SlashDot,
    #[token("*.")]         StarDot,

    #[token("<=")]         LessEq,
    #[token(">=")]         GreaterEq,
    #[token("<>.")]        LessGreaterDot,
    #[token("<=.")]        LessEqDot,
    #[token(">=.")]        GreaterEqDot,
    #[token("<.")]         LessDot,
    #[token(">.")]         GreaterDot,
    #[token("<>")]         LessGreater,
    #[token("<-")]         LessMinus,
    #[token("<")]          Less,
    #[token(">")]          Greater,
    #[token("=")]          Equals,

    #[token("|")]          Pipe,
    #[token("=>")]         Implies,
    #[token("_")]          Underscore,

    #[token(".")]          Dot,
    #[token(",")]          Comma,
    #[token(";")]          Semicolon,
    #[token("(")]          LParen,
    #[token(")")]          RParen,

    #[token("()")]         Unit,

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+ ([eE][-+]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Float(f64),

    #[regex(r"[a-z][a-zA-Z0-9_]*", |lex| lex.slice())]
    Ident(&'a str),
}

#[derive(Debug)]
pub struct Token<'a>{
    pub kind: TokenKind<'a>,
    pub span: Span,
}

pub fn tokenize(input: &'_ str) -> Result<Vec<Token<'_>>, String> {
    let lexer = TokenKind::lexer(input);

    let mut tokens = Vec::new();

    for (result, span) in lexer.spanned(){
        match result{
            Ok(kind) => tokens.push(Token{kind, span}),
            Err(()) => {
                let bad_text = input[span].to_string();
                return Err(format!("Unrecognized character sequence '{}'", bad_text))
            }
        }
    }
    Ok(tokens)
}