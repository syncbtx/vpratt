use vpratt::{Consumed, Rhs, Table, Associativity::Left, TerminalCtx, InfixCtx, PrefixCtx};
use TokenKind::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind{ Num(f64), Plus, Minus,}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr{ Num(f64), Add(Box<Expr>, Box<Expr>), Sub(Box<Expr>, Box<Expr>), Neg(Box<Expr>)}

pub struct Calc<I: Iterator<Item = TokenKind>>{ stream: core::iter::Peekable<I> }

#[vpratt::parser(
    stream = self.stream,
    output = Expr,
    item   = TokenKind,
    token  = TokenKind,
)]
impl<I: Iterator<Item = TokenKind>> Calc<I>{
    pub fn new(stream: I) -> Calc<I>{ Self{stream: stream.peekable()}}

    const TABLE: Table<Self> = Table::new()
        .terminal(Num(0.0), Self::num)
        .infix(20, Left, Plus, Self::add)
        .infix(20, Left, Minus, Self::sub)
        .prefix(30, Minus, Self::negate);

    #[vpratt::handler]
    fn num(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        match ctx.consumed.token{ Num(val) => Ok(Expr::Num(val)), _ => unreachable!() }
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
    fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
    }
}

fn main() {
    // input = "2 + -3 - 4 + 5";
    let tokens = vec![Num(2.0), Plus, Minus, Num(3.0), Minus, Num(4.0), Plus, Num(5.0) ];
    let mut calc = Calc::new(tokens.into_iter());
    let ast = calc.pratt_parse();
    match ast {
        Ok(ast) => println!("AST:\n {:#?}", ast),
        Err(err) => eprintln!("Error: {:#?}", err)
    }
}
