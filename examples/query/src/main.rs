use std::iter::Peekable;
use logos::{Logos, Span};
use vpratt::Associativity::Left;
use vpratt::{GroupCtx, ImpliedCtx, InfixCtx, JuxtCtx, PrefixCtx, Table, TerminalCtx};
use crate::TokenKind::*;

#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\f\r\n]")]
pub enum TokenKind<'a>{
    #[regex(r"[a-zA-Z0-9_]+", |lex| lex.slice())]
    Word(&'a str),

    #[regex(r#""[^"]*""#, |lex| &lex.slice()[1..lex.slice().len() - 1])]
    Quoted(&'a str),

    #[token("AND")] And,
    #[token("OR")]  Or,
    #[token("NOT")] Not,
    #[token("(")]   LParen,
    #[token(")")]   RParen,
    #[token(":")]   Colon,
    Error
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query<'a> {
    kind: QueryKind<'a>,
    pub(crate) span: Span,
}
#[derive(Debug, Clone, PartialEq)]
pub enum QueryKind<'a> {
    /// A bare word term — matches anywhere in the document.
    Term(&'a str),

    /// An exact phrase match — `"rust parser"`.
    Phrase(&'a str),

    /// A field-scoped match — `title:rust` or `author:"john doe"`.
    Field { name: Box<Query<'a>>, value: Box<Query<'a>> },

    /// Boolean NOT — `NOT draft`.
    Not(Box<Query<'a>>),

    /// Boolean AND — `rust AND parser`.
    And(Box<Query<'a>>, Box<Query<'a>>),

    /// Boolean OR — `rust OR python`.
    Or(Box<Query<'a>>, Box<Query<'a>>),
}

fn tokenize(src: &'_ str) -> impl Iterator<Item = (TokenKind<'_>, Span)> {
    TokenKind::lexer(src).spanned().map(|(result, span)| (result.unwrap_or(TokenKind::Error), span))
}

#[derive(Debug)]
pub struct QueryError {
    pub message: String,
    pub span:    logos::Span,
}

impl<'a> From<vpratt::VprattError<(TokenKind<'a>, Span), TokenKind<'a>>> for QueryError {
    fn from(err: vpratt::VprattError<(TokenKind<'a>, Span), TokenKind<'a>>) -> Self {
        match err {
            vpratt::VprattError::UnexpectedEOF => QueryError {
                message: "unexpected end of query".into(),
                span:    0..0,
            },
            vpratt::VprattError::UnexpectedToken(t) => QueryError {
                message: format!("unexpected token `{:?}`", t.0),
                span:    t.1,
            },
            vpratt::VprattError::UnmatchedDelimiter(_, t) => QueryError {
                message: "unmatched `(` — missing closing `)`".into(),
                span:    t.1,
            },
            vpratt::VprattError::ExpectedTokenMismatch(expected, actual) => QueryError {
                message: format!("expected `{:?}`, found `{:?}`", expected, actual.0),
                span:    actual.1,
            },
        }
    }
}

pub struct QueryParser<'a, I: Iterator<Item = (TokenKind<'a>, Span)>>{
    stream: Peekable<I>,
}

impl<'a, I: Iterator<Item = (TokenKind<'a>, Span)>> QueryParser<'a, I>{
    pub fn new(iter: I) -> Self{ Self{stream: iter.peekable()} }
}

#[vpratt::parser(
    stream  = self.stream,
    item    = (TokenKind<'a>, Span),
    token   = TokenKind<'a>,
    output  = Query<'a>,
    error   = QueryError,
    extract = |t: &(TokenKind<'_>, Span)| t.0.clone()
)]
impl<'a, I: Iterator<Item = (TokenKind<'a>, Span)>> QueryParser<'a, I>{
    const TABLE: Table<Self> = Table::new()
        // atomic tokens, prefix position
        // when I say prefix position map that mentally to the nud phase of the Pratt algorithm
        // infix -> led phase
        // postfix is an infix affix with a null rhs
        .terminal(Word(""),         Self::terminals)
        .terminal(Quoted(""),       Self::terminals)
        .terminal(TokenKind::Error, Self::lex_error)

        // if LParen appears in a prefix positon, we assume we will encounter the RParen later in the stream, the handler will parse the enclosed expression and then consume the RParen for you
        .group(LParen, RParen,      Self::group)

        // regular infix operators
        .infix(10, Left, Or,   Self::binary)
        .infix(20, Left, And,  Self::binary)

        // if any of these tokens appear in an infix position, the implicit_and will parse it as if an AND token was inserted just in front of the triggering token
        .implied(20, Left, Word(""),   Self::implicit_and)
        .implied(20, Left, Quoted(""), Self::implicit_and)
        .implied(20, Left, Not,        Self::implicit_and)

        .juxt(20, LParen, RParen, Self::juxt_and)

        .prefix(30, Not,          Self::negate)

        .infix(40, Left, Colon,   Self::binary)

    ;

    #[vpratt::handler]
    fn terminals(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        match ctx.consumed.token.0{
            Word(word)   => Ok(Query{kind: QueryKind::Term(word),   span: ctx.consumed.token.1}),
            Quoted(word) => Ok(Query{kind: QueryKind::Phrase(word), span: ctx.consumed.token.1}),
            _ => unreachable!()
        }
    }

    #[vpratt::handler]
    fn lex_error(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self> {
        Err(QueryError {
            message: "unrecognised character in query".into(),
            span:    ctx.consumed.token.1,
        })
    }

    #[vpratt::handler]
    fn group(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self>{
        let (mut inner, closing) = ctx.enclosed.parse(self)?;
        inner.span = ctx.consumed.token.1.start..closing.1.end;
        Ok(inner)
    }

    #[vpratt::handler]
    fn binary(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let rhs = ctx.rhs.parse(self)?;
        let span = ctx.lhs.span.start..rhs.span.end;
        let kind = match ctx.consumed.token.0{
            And   => QueryKind::And(Box::new(ctx.lhs), Box::new(rhs)),
            Or    => QueryKind::Or(Box::new(ctx.lhs), Box::new(rhs)),
            Colon => {
                let name = match ctx.lhs.kind{
                    QueryKind::Term(_) => ctx.lhs,
                    _ => return Err(QueryError {
                        message: "field name must be a bare word (e.g. `title:`)".into(),
                        span:    ctx.lhs.span,
                    }),
                };
                QueryKind::Field {name: Box::new(name), value: Box::new(rhs)}
            }
            _     => unreachable!()
        };
        Ok(Query{kind, span})
    }

    #[vpratt::handler]
    fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self>{
        let right = ctx.rhs.parse(self)?;
        let span = ctx.consumed.token.1.start..right.span.end;
        let kind = QueryKind::Not(Box::new(right));
        Ok(Query{kind, span})
    }

    #[vpratt::handler]
    fn implicit_and(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self>{
        let expr = ctx.seed.parse(self, ctx.consumed.token)?;
        let full = ctx.resume.parse(self, expr)?;
        let span = ctx.lhs.span.start..full.span.end;
        let kind = QueryKind::And(Box::new(ctx.lhs), Box::new(full));
        Ok(Query{kind, span})
    }

    #[vpratt::handler]
    fn juxt_and(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self>{
        let (inner, closing) = ctx.enclosed.parse(self)?;
        let full = ctx.resume.parse(self, inner)?;
        let span = ctx.lhs.span.start..closing.1.end;
        let kind = QueryKind::And(Box::new(ctx.lhs), Box::new(full));
        Ok(Query{kind, span})
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: query <expression>");
        std::process::exit(1);
    }
    let query = args.join(" ");

    let mut parser = QueryParser::new(tokenize(&query));
    match parser.pratt_parse() {
        Ok(ast)  => println!("\nSource: {}\nAST:    {:#?}\n", query, ast),
        Err(err) => println!("Error:  {:?}\n", err),
    }
}
