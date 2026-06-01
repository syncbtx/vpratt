use core::iter::Peekable;
use core::ops::Range;
use vpratt::{Associativity::{Left, Right}, GroupCtx, ImpliedCtx, InfixCtx, JuxtCtx, PostfixCtx, PrefixCtx, TerminalCtx};
use logos::Logos;
use TokenKind::*;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"[ \t\n\f]+")]
pub enum TokenKind {
    #[regex(r"[0-9]+(?:\.[0-9]+)?")]
    Num,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    #[regex(r"d/d[a-zA-Z_][a-zA-Z0-9_]*")]
    Leibniz,

    #[token("sin")]    Sin,
    #[token("cos")]    Cos,
    #[token("tan")]    Tan,
    #[token("arcsin")] Asin,
    #[token("arccos")] Acos,
    #[token("arctan")] Atan,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("^")] Caret,
    #[token("!")] Bang,
    #[token("'")] Prime,

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
#[vpratt::token(kind)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Fact(Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Asin(Box<Expr>),
    Acos(Box<Expr>),
    Atan(Box<Expr>),
    DerivRespectTo { var: String, expr: Box<Expr> },
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

pub struct MathParser<I: Iterator<Item = Token>> { pub stream: Peekable<I>,}

impl<I: Iterator<Item = Token>> MathParser<I> {
    pub fn new(lexer: I) -> Self {
        Self { stream: lexer.peekable() }
    }
}

#[vpratt::parser(
    stream = self.stream,
    item = Token,                 // type of item yielded by the stream iterator
    output = Expr,                // custom AST type
    entry = parse_math,           // custom entry name for the main pratt parse function
    error = MathError,            // custom error type, must
)]
impl<I: Iterator<Item = Token>> MathParser<I> {

    const TABLE: vpratt::Table<Self> = vpratt::Table::new()
        .terminal(Num,   Self::num)
        .terminal(Ident, Self::var)

        .group(         LParen, RParen,     Self::group)

        .infix(40, Left,  Plus,        Self::add)
        .infix(40, Left,  Minus,       Self::sub)

        .infix(  50, Left,  Star,      Self::mul)
        .infix(  50, Left,  Slash,     Self::div)
        .infix(  50, Left,  Percent,   Self::modulo)

        .prefix( 60, Minus,            Self::negate)
        .prefix( 60, Sin,              Self::trig)
        .prefix( 60, Cos,              Self::trig)
        .prefix( 60, Tan,              Self::trig)
        .prefix( 60, Asin,             Self::trig)
        .prefix( 60, Acos,             Self::trig)
        .prefix( 60, Atan,             Self::trig)
        .prefix( 60, Leibniz,          Self::leibniz)


        .juxt(   70, LParen, RParen,   Self::implicit_mul)
        .implied(70, Left,   Num,      Self::implied_mul)
        .implied(70, Left,   Ident,    Self::implied_mul)
        .implied(70, Left,   Sin,      Self::implied_mul)
        .implied(70, Left,   Cos,      Self::implied_mul)
        .implied(70, Left,   Tan,      Self::implied_mul)
        .implied(70, Left,   Asin,     Self::implied_mul)
        .implied(70, Left,   Acos,     Self::implied_mul)
        .implied(70, Left,   Atan,     Self::implied_mul)
        .implied(70, Left,   Leibniz,  Self::implied_mul)


        .infix(80,  Right, Caret, Self::power)

        .postfix(90, Bang,  Self::factorial)
    ;

    #[vpratt::handler]
    fn trig(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
        let arg = Box::new(ctx.rhs.parse(self)?);
        let kind = match ctx.consumed.token.kind {
            Sin  => Expr::Sin(arg),
            Cos  => Expr::Cos(arg),
            Tan  => Expr::Tan(arg),
            Asin => Expr::Asin(arg),
            Acos => Expr::Acos(arg),
            Atan => Expr::Atan(arg),
            _    => unreachable!(),
        };
        Ok(kind)
    }

    #[vpratt::handler]
    fn num(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        let val = ctx.consumed.token.lexeme.parse::<f64>().map_err(|_| MathError {
            message: "Invalid float".into(),
            span: ctx.consumed.token.span,
        })?;
        Ok(Expr::Num(val))
    }

    #[vpratt::handler]
    fn var(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Var(ctx.consumed.token.lexeme))
    }

    #[vpratt::handler]
    fn group(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self>{
        let (inner, _) = ctx.enclosed.parse(self)?;
        Ok(inner)
    }

    #[vpratt::handler]
    fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn leibniz(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
        let var = ctx.consumed.token.lexeme[3..].to_string();
        let expr = Box::new(ctx.rhs.parse(self)?);
        Ok(Expr::DerivRespectTo { var, expr })
    }

    #[vpratt::handler]
    fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn sub(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Sub(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn mul(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn div(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Div(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn modulo(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Mod(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn power(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Pow(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn factorial(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Fact(Box::new(ctx.lhs)))
    }

    #[vpratt::handler]
    fn implied_mul(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self>{
        let seeded = ctx.seed.parse(self, ctx.consumed.token)?;
        let full = ctx.resume.parse(self, seeded)?;
        Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(full)))
    }

    #[vpratt::handler]
    fn implicit_mul(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self>{
        let (inner, _) = ctx.enclosed.parse(self)?;
        let node = Expr::Mul(Box::new(ctx.lhs), Box::new(inner));
        ctx.resume.parse(self, node)
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

    fn num(n: f64) -> Expr { Expr::Num(n) }
    fn var(s: &str) -> Expr { Expr::Var(s.to_string()) }
    fn add(l: Expr, r: Expr) -> Expr { Expr::Add(Box::new(l), Box::new(r)) }
    fn sub(l: Expr, r: Expr) -> Expr { Expr::Sub(Box::new(l), Box::new(r)) }
    fn mul(l: Expr, r: Expr) -> Expr { Expr::Mul(Box::new(l), Box::new(r)) }
    fn div(l: Expr, r: Expr) -> Expr { Expr::Div(Box::new(l), Box::new(r)) }
    fn pow(l: Expr, r: Expr) -> Expr { Expr::Pow(Box::new(l), Box::new(r)) }
    fn neg(e: Expr) -> Expr { Expr::Neg(Box::new(e)) }
    fn fact(e: Expr) -> Expr { Expr::Fact(Box::new(e)) }
    fn sin(e: Expr) -> Expr { Expr::Sin(Box::new(e)) }
    fn cos(e: Expr) -> Expr { Expr::Cos(Box::new(e)) }
    fn deriv(v: &str, e: Expr) -> Expr { Expr::DerivRespectTo { var: v.to_string(), expr: Box::new(e) } }

    // --- Parser Runner ---
    fn parse(input: &str) -> Expr {
        let tokens = tokenize(input);
        let mut parser = MathParser::new(tokens.into_iter());
        parser.parse_math().expect(&format!("Failed to parse: {}", input))
    }

    #[test]
    fn test_math_lang_parser() {
         assert_eq!(
            parse("2 + 3 * 4 ^ 2"),
            add(num(2.0), mul(num(3.0), pow(num(4.0), num(2.0))))
        );

        assert_eq!(
            parse("100 - 50 - 25"),
            sub(sub(num(100.0), num(50.0)), num(25.0))
        );

        assert_eq!(
            parse("2 ^ 3 ^ 2"),
            pow(num(2.0), pow(num(3.0), num(2.0)))
        );

        assert_eq!(
            parse("2x"),
            mul(num(2.0), var("x"))
        );

        assert_eq!(
            parse("(x + 1)(x - 1)"),
            mul(add(var("x"), num(1.0)), sub(var("x"), num(1.0)))
        );

        // Query  :  4 sin(x) cos(x)
        // Output :  4 * sin(x * cos(x))
        // WARNING: maybe this is not the conventional/intended behavior but this
        // shows that precedence cannot solve all semantic ambiguity
        // this is a result of implicit multiplication binding tighter than trig functions(70 > 60)
        // it is a delicate balance which never settles, just what is most convenient
        assert_eq!(
            parse("4 sin(x) cos(x)"),
            mul(num(4.0), sin(mul(var("x"), cos(var("x")))))
        );

        // Query  :  4 sin(x) * cos(x)
        // Output :  4 * sin(x) * cos(x)
        // NOTE: implicit factors e.g. 2x bind tighter that trig functions(70 > 60):
        // to be safe always use explicit multiplication for two trig functions multiplication
        assert_eq!(
            parse("4 sin(x) * cos(x)"),
            mul(mul(num(4.0), sin(var("x"))), cos(var("x")))
        );

        assert_eq!(
            parse("1 / 2x"),
            div(num(1.0), mul(num(2.0), var("x")))
        );

        assert_eq!(
            parse("-3!"),
            neg(fact(num(3.0)))
        );

        assert_eq!(
            parse("sin -x"),
            sin(neg(var("x")))
        );

        assert_eq!(
            parse("sin x!"),
            sin(fact(var("x")))
        );

        assert_eq!(
            parse("d/dtime (distance / time)"),
            deriv("time", div(var("distance"), var("time")))
        );

        assert_eq!(
            parse("d/dx x^2 + 5x"),
            add(deriv("x", pow(var("x"), num(2.0))), mul(num(5.0), var("x")))
        );

        assert_eq!(
            parse("d/dx (x^2 + 5x)"),
            deriv("x", add(pow(var("x"), num(2.0)), mul(num(5.0), var("x"))))
        );
    }
}

