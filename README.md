# vpratt

**If you can read a precedence table, you can write a parser.**

[![Crates.io](https://img.shields.io/crates/v/vpratt.svg)](https://crates.io/crates/vpratt)
[![Docs.rs](https://docs.rs/vpratt/badge.svg)](https://docs.rs/vpratt)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

![vpratt banner](./vpratt%20banner%20dark.svg)

Forget giant `match` statements, left-recursion workarounds, and recursive descent boilerplate.
`vpratt` is a zero-allocation, `#![no_std]` framework for building strict $\mathcal{O}(N)$ Pratt parsers in Rust.
You control the domain types, the memory management model, and additional state — `vpratt` runs the algorithm.

---

## 1. Centralised Precedence Table

Define your grammar in a declarative table. Precedence numbers live in exactly one place.
If you've ever looked at a precedence table online, writing a `vpratt` table will feel exactly like that.
The format you declare mathematically guarantees the capabilities your handler receives.

```rust
const TABLE: Table<Self> = Table::new()
    .terminal(Num(0.0),      Self::num)
    .prefix(30, Minus,       Self::negate)
    .infix( 20, Left, Plus,  Self::add)
    .infix( 20, Left, Minus, Self::sub);

// `.infix()` guarantees you receive `lhs`, the `consumed` operator, and an `rhs` capability.
#[vpratt::handler]
fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Add(
        Box::new(ctx.lhs),
        Box::new(ctx.rhs.parse(self)?) // safely parses using Plus's right-binding-power
    ))
}
```

## 2. Native Juxtaposition. No Lexer Hacks.

Handle `2x` or `f a b` natively. No phantom tokens, no unlimited lookaheads. Re-inject nodes directly into the state machine in a single pass.

```rust
.implied(70, Left, Ident(""), Self::application)

#[vpratt::handler]
fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
    let token = ctx.consumed.token;

    // seed: feed the token back into the prefix router to parse the first argument
    let arg  = ctx.seed.parse(self, token)?;

    // resume: push the result back into the infix loop to chain remaining arguments
    let full = ctx.resume.parse(self, arg)?;

    Ok(Expr::Apply(Box::new(ctx.lhs), Box::new(full)))
}
```

## 3. Beyond Math: Structural Parsing

`vpratt` isn't just for calculators. Use `.structural()` and the `CtxUtils` trait to parse complex keywords, lists, and control flow without leaving the safe context environment. These additional features serve as a safe extension of vpratt into the recursive descent domain.

```rust
.structural(If, Self::parse_if)

#[vpratt::handler]
fn parse_if(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
    // sub: temporarily drop precedence to 0 to parse a fresh expression
    let cond = self.arena.alloc(ctx.sub.parse(self)?);

    // expect: strictly consume the next token or fail with a typed error
    ctx.expect(self, Then)?;
    let then = self.arena.alloc(ctx.sub.parse(self)?);

    ctx.expect(self, Else)?;
    let else_ = self.arena.alloc(ctx.sub.parse(self)?);

    Ok(Expression {
        kind: ExprKind::If { cond, then, else_ },
        span: Span::new(cond.span.start, else_.span.end),
        ty:   None,
    })
}
```

*Need to parse argument lists? `ctx.separated(self, Ident(""), Comma, RParen)?` does it safely.*

## 4. Bring Your Own Types, Memory, and State

`vpratt` is strictly zero-allocation and `no_std` compatible. It gets completely out of your way.
Our examples use `Box` for convenience, but you are free to use arena allocators (like `bumpalo`), `Rc`, or any custom AST representation.

Because every handler receives `&mut self`, you can seamlessly pass and mutate any additional state without globals or convoluted context objects. Symbol table, diagnostic accumulator, arena — just put it in your parser struct.

```rust
#[vpratt::parser(
    stream  = self.stream,
    output  = Expression<'a>,
    item    = Token<'a>,
    token   = TokenKind<'a>,
    error   = Diagnostic<'a>,
    extract = |t: &Token<'a>| t.kind.clone(),
)]
impl<'a> Parser<'a> {
    #[vpratt::handler]
    fn some_handler(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        let node = self.arena.alloc(ctx.rhs.parse(self)?);
        self.diagnostics.push(Warning::NewNodeAllocated);
        self.symbol_table.register("foo", node);
        Ok(node)
    }
}
```

```rust
// Map the engine's mechanical failures to your diagnostics — once, in one place
impl From<VprattError<Token<'_>, TokenKind<'_>>> for Diagnostic<'_> {
    fn from(err: VprattError<Token, TokenKind>) -> Self {
        match err {
            VprattError::UnexpectedEOF                           => Diagnostic::eof(),
            VprattError::UnexpectedToken(t)                      => Diagnostic::unexpected(t.span),
            VprattError::UnmatchedDelimiter(expected, actual)    => Diagnostic::unmatched(actual.span),
            VprattError::ExpectedTokenMismatch(expected, actual) => Diagnostic::expected(expected, actual.span),
        }
    }
}
```

---

## Full Example

<details>
<summary>Self-contained arithmetic parser — copy, paste, run</summary>

```rust
use core::iter::Peekable;
use vpratt::{Table, Associativity::Left};
use vpratt::{GroupCtx, InfixCtx, PrefixCtx, TerminalCtx};
use TokenKind::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind { Num(f64), Plus, Minus, Star, Slash, LParen, RParen }

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
}

pub struct Calc<I: Iterator<Item = TokenKind>> {
    stream: Peekable<I>,
}

#[vpratt::parser(
    stream = self.stream,
    output = Expr,
    item   = TokenKind,
    token  = TokenKind,
)]
impl<I: Iterator<Item = TokenKind>> Calc<I> {
    pub fn new(stream: I) -> Self {
        Self { stream: stream.peekable() }
    }

    const TABLE: Table<Self> = Table::new()
        .terminal(Num(0.0),     Self::num)
        .group(LParen, RParen,  Self::grouped)
        .prefix(30, Minus,      Self::negate)
        .infix(10, Left, Plus,  Self::add)
        .infix(10, Left, Minus, Self::sub)
        .infix(20, Left, Star,  Self::mul)
        .infix(20, Left, Slash, Self::div);

    #[vpratt::handler]
    fn num(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self> {
        match ctx.consumed.token { Num(v) => Ok(Expr::Num(v)), _ => unreachable!() }
    }

    #[vpratt::handler]
    fn grouped(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self> {
        Ok(ctx.enclosed.parse(self)?.0)
    }

    #[vpratt::handler]
    fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn sub(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Sub(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn mul(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn div(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
        Ok(Expr::Div(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
    }
}

fn main() {
    // "2 + -3 * (4 - 1)"
    let tokens = vec![
        Num(2.0), Plus, Minus, Num(3.0),
        Star, LParen, Num(4.0), Minus, Num(1.0), RParen,
    ];
    match Calc::new(tokens.into_iter()).pratt_parse() {
        Ok(ast)  => println!("{:#?}", ast),
        Err(err) => eprintln!("{:#?}", err),
    }
}

// Add(
//     Num(2.0),
//     Mul(
//         Neg(Num(3.0)),
//         Sub(Num(4.0), Num(1.0)),
//     ),
// )
```

</details>

---

## Installation

```sh
cargo add vpratt
```

## Going Further

The [`examples/math-lang`](./examples/math-lang) directory is a complete, runnable arithmetic language demonstrating advanced juxtaposition and robust error recovery:

```sh
cargo run --example math-lang
```

For the full reference — every format, every context type, every capability token, and the `CtxUtils` API — see **[DOC.md](./DOC.md)**.