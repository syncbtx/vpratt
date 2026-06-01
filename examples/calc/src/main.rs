use vpratt::{Table, Associativity::Left, TerminalCtx, InfixCtx, PrefixCtx, PostfixCtx};
use TokenKind::*;
use vpratt::Associativity::Right;

#[derive(Debug, Clone, Copy, PartialEq)]
#[vpratt::token(self)]
pub enum TokenKind{ Num(f64), Plus, Minus, Star, Slash, Percent, Caret, Bang}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr{
    Num(f64),
    Binary(Box<Expr>, BinaryOp,  Box<Expr>),
    Neg(Box<Expr>),
    Fact(Box<Expr>)
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp{ Add, Sub, Mul, Div, Mod, Pow,}

impl BinaryOp{
    pub fn from_token_kind(kind: TokenKind) -> Self {
        match kind {
            Plus => BinaryOp::Add,
            Minus => BinaryOp::Sub,
            Star => BinaryOp::Mul,
            Slash => BinaryOp::Div,
            Percent => BinaryOp::Mod,
            Caret => BinaryOp::Pow,
            _ => unreachable!()
        }
    }
}



pub struct Calc<I: Iterator<Item = TokenKind>>{ stream: core::iter::Peekable<I> }

#[vpratt::parser(
    stream = self.stream,
    item   = TokenKind,
    output = Expr,
)]
impl<I: Iterator<Item = TokenKind>> Calc<I>{
    pub fn new(stream: I) -> Calc<I>{ Self{stream: stream.peekable()}}

    const TABLE: Table<Self> = Table::new()
        .terminal(Num(0.0), Self::num)
        .infix(20, Left, Plus, Self::binary)
        .infix(20, Left, Minus, Self::binary)
        .infix(30, Left, Star, Self::binary)
        .infix(30, Left, Slash, Self::binary)
        .infix(30, Left, Percent, Self::binary)
        .prefix(40, Minus, Self::negate)
        .infix(50, Right, Caret, Self::binary)
        .postfix(60, Bang, Self::factorial)
    ;

    #[vpratt::handler]
    fn num(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        match ctx.consumed.token{ Num(val) => Ok(Expr::Num(val)), _ => unreachable!() }
    }
    #[vpratt::handler]
    fn binary(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let op = BinaryOp::from_token_kind(ctx.consumed.token);
        Ok(Expr::Binary(Box::new(ctx.lhs), op, Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
    }
    #[vpratt::handler]
    fn factorial(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Fact(Box::new(ctx.lhs)))
    }
}

fn main() {
    let input = "2 + -3 % 4 + 5 ^ 3 - 2!";
    let tokens = vec![Num(2.0), Plus, Minus, Num(3.0), Percent, Num(4.0), Plus, Num(5.0), Caret, Num(3.0), Minus, Num(2.0), Bang ];
    let mut calc = Calc::new(tokens.into_iter());
    let ast = calc.pratt_parse();
    match ast {
        Ok(ast) => println!("Source: {}\nAST:\n{:#?}", input, ast),
        Err(err) => eprintln!("Error: {:#?}", err)
    }
}
