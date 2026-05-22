use core::iter::Peekable;
use core::ops::Range;
use vpratt::{Consumed, Rhs, Enclosed, Resume, Associativity::{Left, Right}, VprattCore, Seed};
use logos::Logos;
use TokenKind::*;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t\n\f]+")]
pub enum TokenKind {
    #[regex(r"[0-9]+(?:\.[0-9]+)?")]
    Num,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("^")] Caret,
    #[token("!")] Bang,
    #[token("(")] LParen,
    #[token(")")] RParen,

    EOF
}

pub fn tokenize(input: &str) -> Vec<Token> {
    let mut lexer = TokenKind::lexer(input);
    let mut tokens = Vec::new();

    while let Some(result) = lexer.next() {
        let kind = result.expect("Lexer encountered an invalid token!");

        tokens.push(Token {
            kind,
            lexeme: lexer.slice().to_string(),
            span: lexer.span(),
        });
    }
    tokens.push(Token {
        kind: TokenKind::EOF,
        lexeme: "".to_string(),
        span: input.len()..input.len(),
    });

    tokens
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Group(Box<Expr>, Range<usize>),             // this abstraction might be desirable in some contexts
    Neg(Box<Expr>),                             // -x
    Fact(Box<Expr>),                            // x!
    Pow(Box<Expr>, Box<Expr>),                  // x^y
    Add(Box<Expr>, Box<Expr>),                  // x + y
    Sub(Box<Expr>, Box<Expr>),                  // x - y
    Mul(Box<Expr>, Box<Expr>),                  // x * y
    Div(Box<Expr>, Box<Expr>),                  // x / y
    Mod(Box<Expr>, Box<Expr>),                  // x % y
}

#[derive(Debug)]
pub struct MathError {
    pub message: String,
    pub span: Range<usize>,
}

impl From<vpratt::VprattError<Token, TokenKind>> for MathError {
    fn from(err: vpratt::VprattError<Token, TokenKind>) -> Self {
        match err {
            vpratt::VprattError::UnexpectedEOF => MathError {
                message: "Unexpected end of file while parsing expression.".into(),
                span: 0..0
            },
            vpratt::VprattError::UnexpectedToken(t) => MathError {
                message: format!("Unexpected token: {:?}", t.kind),
                span: t.span
            },
            vpratt::VprattError::UnmatchedDelimiter (expected, found) => MathError {
                message: format!("Expected closing delimiter {:?}, but found {:?}", expected, found.kind),
                span: found.span
            },
            vpratt::VprattError::ExpectedTokenMismatch(expected, found) => MathError{
                message: format!("Expected {:?} but found {:?}", expected, found.kind),
                span: found.span
            }
        }
    }
}

pub struct MathParser<I: Iterator<Item = Token>> {
    pub stream: Peekable<I>,
}

impl<I: Iterator<Item = Token>> MathParser<I> {
    pub fn new(lexer: I) -> Self {
        Self { stream: lexer.peekable() }
    }
}

#[vpratt::parser(
    stream = self.stream,
    output = Expr,                // custom AST type
    item = Token,                 // type of item yielded by the stream iterator
    entry = parse_math,           // custom entry name for the main pratt parse function
    token = TokenKind,            // internally referred to as PrattToken: the routing token, extracted from the item type
    error = MathError,            // custom error type, must
    extract = |t: &Token| t.kind, // can also be: extract = get_kind, {fn get_kind(t: &Token) -> TokenKind { t.kind }?}
)]
impl<I: Iterator<Item = Token>> MathParser<I> {

    const TABLE: vpratt::Table<Self> = vpratt::Table::new()
        .terminal(Num,   Self::num)
        .terminal(Ident, Self::var)

        .group(LParen, RParen, Self::group)

        .infix(40, Left, Plus,  Self::add)
        .infix(40, Left, Minus, Self::sub)
        .infix(50, Left, Star,  Self::mul)
        .infix(50, Left, Slash, Self::div)

        .juxt(60, LParen, RParen, Self::implicit_mul)
        .implied(60, Left, Num, Self::implied_mul)
        .implied(60, Left, Ident, Self::implied_mul)

        .prefix(70, Minus, Self::negate)
        .infix(80, Right, Caret, Self::power)
        .postfix(90, Bang, Self::factorial);


    #[vpratt::handler]
    fn num(&mut self, token: Consumed<Token>) -> vpratt::Result<Self>{
        let val = token.token.lexeme.parse::<f64>().map_err(|_| MathError {
            message: "Invalid float".into(),
            span: token.token.span,
        })?;
        Ok(Expr::Num(val))
    }

    #[vpratt::handler]
    fn var(&mut self, token: Consumed<Token>) -> vpratt::Result<Self>{
        Ok(Expr::Var(token.token.lexeme))
    }

    #[vpratt::handler]
    fn group(&mut self, open: Consumed<Token>, enclosed: Enclosed<Self>) -> vpratt::Result<Self>{
        let (inner, close) = enclosed.parse(self)?;
        Ok(Expr::Group(Box::new(inner), open.token.span.start .. close.span.end))
    }

    #[vpratt::handler]
    fn negate(&mut self, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Neg(Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn power(&mut self, lhs: Expr, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Pow(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn mul(&mut self, lhs: Expr, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Mul(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn div(&mut self, lhs: Expr, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Div(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn add(&mut self, lhs: Expr, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Add(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn sub(&mut self, lhs: Expr, _op: Consumed<Token>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Sub(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn factorial(&mut self, lhs: Expr, _op: Consumed<Token>) -> vpratt::Result<Self>{
        Ok(Expr::Fact(Box::new(lhs)))
    }

    #[vpratt::handler]
    fn implied_mul(
        &mut self,
        lhs: Expr,
        c: Consumed<Token>,
        seed: Seed<Self>,
        resume: Resume<Self>
    ) -> vpratt::Result<Self>{
        let right_start = seed.parse(self, c.token)?;

        let right_full = resume.parse(self, right_start)?;

        Ok(Expr::Mul(Box::new(lhs), Box::new(right_full)))
    }

    #[vpratt::handler]
    fn implicit_mul(
        &mut self,
        lhs: Expr,
        _: Consumed<Token>,
        enclosed: Enclosed<Self>,
        resume: Resume<Self>
    ) -> vpratt::Result<Self>{
        let (inner, _) = enclosed.parse(self)?;
        let node = Expr::Mul(Box::new(lhs), Box::new(inner));
        resume.parse(self, node)
    }
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let src = args[0].clone();
    let tokens = tokenize(&src);

    let mut parser = MathParser::new(tokens.into_iter());

    match parser.parse_math() {
        Ok(ast) => {
            println!("{}\nAST:\n{:#?}", src ,ast);
        }
        Err(err) => {
            eprintln!("Parse Error: {} (Span: {:?})", err.message, err.span);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(kind: TokenKind, lexeme: &str, span: Range<usize>) -> Token {
        Token { kind, lexeme: lexeme.to_string(), span }
    }

    fn parse(tokens: Vec<Token>) -> Result<Expr, MathError> {
        let mut parser = MathParser::new(tokens.into_iter());
        parser.parse_math()
    }

    #[test]
    fn test_standard_precedence() {
        let tokens = vec![
            t(Num, "2", 0..1),
            t(Plus, "+", 2..3),
            t(Num, "3", 4..5),
            t(Star, "*", 6..7),
            t(Num, "4", 8..9),
            t(EOF, "", 9..9),
        ];

        let expected = Expr::Add(
            Box::new(Expr::Num(2.0)),
            Box::new(Expr::Mul(
                Box::new(Expr::Num(3.0)),
                Box::new(Expr::Num(4.0))
            ))
        );

        assert_eq!(parse(tokens).unwrap(), expected);
    }

    #[test]
    fn test_implied_multiplication_with_trailing_operator() {
        // Simulating: 2x^3 (Proves `Resume` works perfectly)
        let tokens = vec![
            t(Num, "2", 0..1),
            t(Ident, "x", 1..2),
            t(Caret, "^", 2..3),
            t(Num, "3", 3..4),
            t(EOF, "", 4..4),
        ];

        let expected = Expr::Mul(
            Box::new(Expr::Num(2.0)),
            Box::new(Expr::Pow(
                Box::new(Expr::Var("x".into())),
                Box::new(Expr::Num(3.0))
            ))
        );

        assert_eq!(parse(tokens).unwrap(), expected);
    }

    #[test]
    fn test_engine_error_unexpected_token() {
        // Simulating: 2 + * 3
        let tokens = vec![
            t(Num, "2", 0..1),
            t(Plus, "+", 2..3),
            t(Star, "*", 4..5), // The bad token
            t(Num, "3", 6..7),
            t(EOF, "", 7..7),
        ];

        let result = parse(tokens);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.message, "Unexpected token: Star");
        assert_eq!(err.span, 4..5);
    }

    #[test]
    fn test_engine_error_unmatched_delimiter() {
        let tokens = vec![
            t(Num, "2", 0..1),
            t(LParen, "(", 1..2),
            t(Ident, "a", 2..3),
            t(Plus, "+", 4..5),
            t(Ident, "b", 6..7),
            t(EOF, "", 7..7), // Missing RParen!
        ];

        let result = parse(tokens);

        assert!(result.is_err());
        let err = result.unwrap_err();

        assert_eq!(err.message, "Expected closing delimiter RParen, but found EOF");
        assert_eq!(err.span, 7..7);
    }
}