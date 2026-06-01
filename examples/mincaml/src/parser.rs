use core::iter::Peekable;
use ariadne::{Color, Label, Report, ReportKind};
use logos::Span;
use vpratt::{CtxUtils, ImpliedCtx, InfixCtx, JuxtCtx, PrefixCtx, StructuralCtx, TerminalCtx, Associativity::{Left, Right}};
use crate::lexer::{Token, TokenKind};
use crate::ast::{ExprKind, Expression, UnaryOp, BinaryOp};
use TokenKind::*;
use bumpalo::Bump;
use crate::diagnostics::ParserError;

pub struct MincamlParser<'a, I: Iterator<Item=Token<'a>>>{ stream: Peekable<I>, arena: &'a Bump,}

impl<'a, I: Iterator<Item = Token<'a>>> MincamlParser<'a, I>{
    pub fn new(iter: I, arena: &'a Bump) -> Self{
        Self{ stream: iter.peekable(), arena }
    }
}

#[vpratt::parser(
    stream  =  self.stream,
    item    =  Token<'a>,
    token   =  TokenKind<'a>,
    output  =  Expression<'a>,
    error   =  ParserError<'a>,
    extract =  |t: &Token<'a>| t.kind.clone(),
)]
impl<'a, I: Iterator<Item = Token<'a>>> MincamlParser<'a, I>{
    const TABLE: vpratt::Table<Self> = vpratt::Table::new()
        .terminal(Ident(""),  Self::terminals)
        .terminal(Int(0),     Self::terminals)
        .terminal(Float(0.0), Self::terminals)
        .terminal(True,       Self::terminals)
        .terminal(False,      Self::terminals)
        .terminal(Unit,       Self::terminals)

        .structural(LParen,      Self::parenthesized)
        .structural(If,          Self::parse_if)
        .structural(Let,         Self::parse_let)
        .structural(ArrayCreate, Self::parse_array_create)

        .infix(10, Right, Semicolon,      Self::sequence  )
        .infix(20, Right, LessMinus,      Self::parse_array_set)

        .infix(40, Left,  LessEq,         Self::binary)
        .infix(40, Left,  GreaterEq,      Self::binary)
        .infix(40, Left,  LessGreaterDot, Self::binary)
        .infix(40, Left,  LessEqDot,      Self::binary)
        .infix(40, Left,  GreaterEqDot,   Self::binary)
        .infix(40, Left,  LessDot,        Self::binary)
        .infix(40, Left,  GreaterDot,     Self::binary)
        .infix(40, Left,  LessGreater,    Self::binary)
        .infix(40, Left,  Less,           Self::binary)
        .infix(40, Left,  Greater,        Self::binary)
        .infix(40, Left,  Equals,         Self::binary)

        .infix(50, Left,  Minus,          Self::binary  )
        .infix(50, Left,  MinusDot,       Self::binary  )
        .infix(50, Left,  Plus,           Self::binary  )
        .infix(50, Left,  PlusDot,        Self::binary  )

        .infix(60, Left,  Star,           Self::binary  )
        .infix(60, Left,  StarDot,        Self::binary  )
        .infix(60, Left,  Slash,          Self::binary  )
        .infix(60, Left,  SlashDot,       Self::binary  )

        .prefix(70, Minus,                Self::unary)
        .prefix(70, MinusDot,             Self::unary)
        .prefix(70, Not,                  Self::unary)

        .implied(80, Left, Ident(""),     Self::application)
        .implied(80, Left, Int(0),        Self::application)
        .implied(80, Left, Float(0.0),    Self::application)
        .implied(80, Left, True,          Self::application)
        .implied(80, Left, False,         Self::application)
        .implied(80, Left, Unit,          Self::application)
        .implied(80, Left, ArrayCreate,   Self::application)

        .juxt(80, LParen, RParen,         Self::juxt_application)

        .infix(100, Left, Dot,            Self::parse_array_get);

    #[vpratt::handler]
    fn terminals(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>{
        let span = ctx.consumed.token.span;
        match ctx.consumed.token.kind {
            Ident(val)    => Ok(Expression{kind: ExprKind::Identifier(val), span}),
            Int(val)      => Ok(Expression{kind: ExprKind::Integer(val), span}),
            Float(val)    => Ok(Expression{kind: ExprKind::Float(val), span}),
            True                => Ok(Expression{kind: ExprKind::Boolean(true), span}),
            False               => Ok(Expression{kind: ExprKind::Boolean(false), span}),
            Unit                => Ok(Expression{kind: ExprKind::Unit, span}),
            _ => unreachable!()
        }
    }

    #[vpratt::handler]
    fn parenthesized(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.consumed.token.span.start;
        let first = ctx.sub.parse(self)?;
        if let Some(_) = ctx.accept(self, RParen)?{ return Ok(first) };
        ctx.expect(self, Comma)?;
        let remaining = ctx.separated_sub(self, Comma, RParen)?;
        let elements: Vec<_>  = core::iter::once(&*self.arena.alloc(first))
            .chain(remaining.into_iter().map(|elem| &*self.arena.alloc(elem)))
            .collect();

        let end = match elements.last(){
            Some(expr) => expr.span.end,
            None => ctx.consumed.token.span.end
        };

        let kind = ExprKind::Tuple(elements);
        Ok(Expression{ kind, span: Span{start, end}, })
    }


    #[vpratt::handler]
    fn sequence(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.lhs.span.start;
        let first  = self.arena.alloc(ctx.lhs);
        let second = self.arena.alloc(ctx.rhs.parse(self)?);
        let end = second.span.end;
        let kind = ExprKind::Sequence {first, second};
        Ok(Expression{kind, span: Span{start, end}})
    }

    #[vpratt::handler]
    fn unary(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.consumed.token.span.start;
        let op = match UnaryOp::from_token_kind(ctx.consumed.token.kind, ctx.consumed.token.span){
            Some(op) => op,
            None => unreachable!(),
        };
        let right = self.arena.alloc(ctx.rhs.parse(self)?);
        let kind = ExprKind::Unary{ op, right };
        let end = right.span.end;
        Ok(Expression{ kind, span: Span{start, end} })
    }

    #[vpratt::handler]
    fn binary(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.lhs.span.start;
        let op = match BinaryOp::from_token_kind(ctx.consumed.token.kind, ctx.consumed.token.span){
            Some(op) => op,
            None => unreachable!(),
        };
        let left = self.arena.alloc(ctx.lhs);
        let right = self.arena.alloc(ctx.rhs.parse(self)?);
        let kind = ExprKind::Binary{left, op, right };
        let end = right.span.end;
        Ok(Expression{ kind, span: Span{start, end} })
    }

    #[vpratt::handler]
    fn parse_if(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.consumed.token.span.start;
        let condition = self.arena.alloc(ctx.sub.parse(self)?);
        ctx.expect(self, Then)?;
        let then_expr = self.arena.alloc(ctx.sub.parse(self)?);
        ctx.expect(self, Else)?;
        let else_expr = self.arena.alloc(ctx.sub.parse(self)?);
        let end = else_expr.span.end;
        let kind = ExprKind::If { condition, then_expr, else_expr,};
        Ok(Expression{kind, span: Span{start, end}})
    }

    #[vpratt::handler]
    fn parse_let(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self>{
        enum LetHead<'a> {
            Standard(&'a Expression<'a>),
            Rec{func: &'a Expression<'a>, args: Vec<&'a Expression<'a>>},
            Tuple(Vec<&'a Expression<'a>>),
        }
        let start = ctx.consumed.token.span.start;
        let head = if ctx.accept(self, Recursive)?.is_some(){
            let func = self.arena.alloc(ctx.atom.parse(self)?);
            let args = ctx.series(self, Ident(""), Equals)?;
            let args = args.into_iter().map(|arg| &*self.arena.alloc(arg)).collect::<Vec<_>>();
            LetHead::Rec {func, args}
        }
        else if ctx.accept(self, LParen)?.is_some(){
            let elements = ctx.separated(self, Ident(""), Comma, RParen)?;
            let elements = elements.into_iter().map(|arg| &*self.arena.alloc(arg)).collect::<Vec<_>>();
            LetHead::Tuple(elements)
        }
        else{
            LetHead::Standard(self.arena.alloc(ctx.atom.parse(self)?))
        };

        ctx.expect(self, Equals)?;

        let value = self.arena.alloc(ctx.sub.parse(self)?);

        ctx.expect(self, In)?;

        let body = self.arena.alloc(ctx.sub.parse(self)?);
        let end = body.span.end;

        let kind = match head{
            LetHead::Standard(ident)=>{ExprKind::Let {ident, value, body}},
            LetHead::Tuple(idents) => {ExprKind::LetTuple{ idents, value, body, }},
            LetHead::Rec {func, args} => {ExprKind::LetRec { func, args, value, body,}}
        };

        Ok(Expression{ kind, span: Span{ start, end } })
    }

    #[vpratt::handler]
    fn parse_array_create(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.consumed.token.span.start;
        let size = self.arena.alloc(ctx.atom.parse(self)?);
        let init = self.arena.alloc(ctx.atom.parse(self)?);
        let end = init.span.end;
        let kind = ExprKind::ArrayCreate {size, init};
        Ok(Expression{ kind, span: Span{ start, end } })
    }

    #[vpratt::handler]
    fn parse_array_get(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.lhs.span.start;
        let ident = self.arena.alloc(ctx.lhs);
        let index = self.arena.alloc(ctx.rhs.parse(self)?);
        let end = index.span.end;
        let kind = ExprKind::ArrayGet { ident, index };
        Ok(Expression{ kind, span: Span{start, end}, })
    }

    #[vpratt::handler]
    fn parse_array_set(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>{
        let (ident, index) = match ctx.lhs.kind{
            ExprKind::ArrayGet {ident, index} => (ident, index),
            _ => return Err(ParserError(
                Report::build(ReportKind::Error, ctx.lhs.span.clone())
                    .with_message("the left side of `<-` must be an array access")
                    .with_label(
                        Label::new(ctx.lhs.span.clone())
                            .with_message("expected `array.(index)` here")
                            .with_color(Color::Red)
                    )
                    .finish()
            )),
        };
        let start = ctx.lhs.span.start;
        let value = self.arena.alloc(ctx.rhs.parse(self)?);
        let end = value.span.end;
        let kind = ExprKind::ArraySet{ident, index, value, };
        Ok(Expression{ kind, span: Span{start, end}, })
    }

    #[vpratt::handler]
    fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.lhs.span.start;
        let arg = self.arena.alloc(ctx.seed.parse(self, ctx.consumed.token)?);
        let end = arg.span.end;
        let kind = match ctx.lhs.kind{
            ExprKind::Application{callee, mut args} => {
                args.push(arg);
                ExprKind::Application{ callee, args}
            }
            _ => {
                let callee = self.arena.alloc(ctx.lhs);
                ExprKind::Application{ callee, args: vec![arg]}
            }
        };
        Ok(Expression{ kind, span: Span{start, end} })
    }

    #[vpratt::handler]
    fn juxt_application(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self>{
        let start = ctx.lhs.span.start;
        let (expr, closing) = ctx.enclosed.parse(self)?;
        let end = closing.span.end;
        let inner = self.arena.alloc(expr);
        let kind = match ctx.lhs.kind{
            ExprKind::Application{callee, mut args} => {
                args.push(inner);
                ExprKind::Application{ callee, args}
            }
            _ => {
                let callee = self.arena.alloc(ctx.lhs);
                ExprKind::Application{ callee, args: vec![inner]}
            }
        };
        Ok(Expression{ kind, span: Span{start, end} })
    }
}