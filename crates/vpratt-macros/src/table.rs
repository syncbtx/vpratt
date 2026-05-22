use proc_macro2::Span;
use syn::Expr;

// defines as an IR for the custom DSL

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assoc {
    Left,
    Right,
}

pub struct TableDef {
    pub entries: Vec<Entry>,
}

pub enum Entry {
    Terminal {
        token: Expr,
        handler: Expr,
        span: Span,
    },
    Group {
        open: Expr,
        close: Expr,
        handler: Expr,
        span: Span,
    },
    Structural {
        token: Expr,
        handler: Expr,
        span: Span,
    },
    Prefix {
        bp: Expr,
        token: Expr,
        handler: Expr,
        span: Span,
    },
    Infix {
        bp: Expr,
        assoc: Assoc,
        token: Expr,
        handler: Expr,
        span: Span,
    },
    Postfix {
        bp: Expr,
        token: Expr,
        handler: Expr,
        span: Span,
    },
    Juxt {
        bp: Expr,
        open: Expr,
        close: Expr,
        handler: Expr,
        span: Span,
    },
    Implied {
        bp: Expr,
        assoc: Assoc,
        token: Expr,
        handler: Expr,
        span: Span
    }
}