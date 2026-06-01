use logos::Span;
use crate::ast::BinaryOp::*;
use crate::lexer::TokenKind;
use crate::lexer::TokenKind::*;

#[derive(Debug, Clone)]
pub struct Expression<'a>{
    pub kind: ExprKind<'a>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind<'a>{
    Unit,
    Identifier(&'a str),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Unary{op: UnaryOp, right: &'a Expression<'a>},
    Binary{left: &'a Expression<'a>, op: BinaryOp, right: &'a Expression<'a>},
    Tuple(Vec<&'a Expression<'a>>),
    Let{
        ident: &'a Expression<'a>,
        value: &'a Expression<'a>,
        body : &'a Expression<'a>,
    },
    LetRec{
        func: &'a Expression<'a>,
        args: Vec<&'a Expression<'a>>,
        value: &'a Expression<'a>,
        body : &'a Expression<'a>,
    },
    LetTuple{
        idents: Vec<&'a Expression<'a>>,
        value: &'a Expression<'a>,
        body : &'a Expression<'a>,
    },
    If{
        condition: &'a Expression<'a>,
        then_expr: &'a Expression<'a>,
        else_expr: &'a Expression<'a>,
    },
    Application{
        callee: &'a Expression<'a>,
        args: Vec<&'a Expression<'a>>
    },
    ArrayCreate{
        size: &'a Expression<'a>,
        init: &'a Expression<'a>,
    },
    ArrayGet{
        ident: &'a Expression<'a>,
        index: &'a Expression<'a>,
    },
    ArraySet{
        ident: &'a Expression<'a>,
        index: &'a Expression<'a>,
        value: &'a Expression<'a>,
    },
    Sequence{
        first: &'a Expression<'a>,
        second: &'a Expression<'a>,
    },

}

#[derive(Debug, Clone)]
pub enum UnaryOp{
    Neg(Span),
    FNeg(Span),
    Not(Span)
}

impl UnaryOp{
    pub fn from_token_kind(kind: TokenKind, span: Span) -> Option<UnaryOp>{
        match kind {
            Minus => Some(UnaryOp::Neg(span)),
            MinusDot => Some(UnaryOp::FNeg(span)),
            Not => Some(UnaryOp::Not(span)),
            _ => None
        }
    }
}
#[derive(Debug, Clone)]
pub enum BinaryOp{
    Add(Span), Sub(Span), Mul(Span), Div(Span),
    FAdd(Span), FSub(Span), FMul(Span), FDiv(Span),
    Eq(Span),
    Lt(Span),
    Gt(Span),
    Le(Span),
    Ge(Span),
    FLt(Span),
    FGt(Span),
    FLe(Span),
    FGe(Span),
    Neq(Span),
    FNeq(Span),
}

impl BinaryOp{
    pub fn from_token_kind(kind: TokenKind, span: Span) -> Option<BinaryOp>{
        match kind {
            Plus         => Some(Add(span)),
            Minus        => Some(Sub(span)),
            Star         => Some(Mul(span)),
            Slash        => Some(Div(span)),
            PlusDot      => Some(FAdd(span)),
            MinusDot     => Some(FDiv(span)),
            StarDot      => Some(FMul(span)),
            SlashDot     => Some(FSub(span)),
            Equals       => Some(Eq(span)),
            Less         => Some(Lt(span)),
            Greater      => Some(Gt(span)),
            LessEq       => Some(Le(span)),
            GreaterEq    => Some(Ge(span)),
            LessDot      => Some(FLt(span)),
            GreaterDot   => Some(FGt(span)),
            LessEqDot    => Some(FLe(span)),
            GreaterEqDot => Some(FGe(span)),
            _ => None,
        }
    }
}