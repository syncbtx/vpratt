# vpratt

**If you can read a precedence table, you can write a parser.**

[![Crates.io](https://img.shields.io/crates/v/vpratt.svg)](https://crates.io/crates/vpratt)
[![Docs.rs](https://docs.rs/vpratt/badge.svg)](https://docs.rs/vpratt)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

![vpratt banner](./vpratt%20banner%20dark.svg)

Forget giant `match` statements, left-recursion workarounds, and recursive descent boilerplate.
`vpratt` is a zero-allocation, `#![no_std]` framework for building strict $\mathcal{O}(N)$ Pratt parsers in Rust.<br>
 You control the domain types, the memory management model, and additional state — `vpratt` runs the algorithm.

---

## 1. Centralised Precedence Table
Define your grammar in a declarative table. Precedence numbers live in exactly one place.<br>
If you've ever looked at a precedence table online, writing a `vpratt` table will feel exactly like that.<br>
The format you declare mathematically guarantees the capabilities your handler receives. <br>

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

## 2. Native juxtaposition. No lexer hacks.
Handle `2x` or `f a b` natively. No lexer hacks (like phantom `*` tokens) or unlimited lookaheads. Re-inject nodes directly into the state machine in a single pass.

```rust
.implied(70, Left, Ident(""), Self::application)

#[vpratt::handler]
fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
    // seed: feed the token back into the prefix router to parse the first argument
    let arg  = ctx.seed.parse(self, ctx.consumed.token)?;

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
    let cond  = self.arena.alloc(ctx.sub.parse(self)?);
    
    // expect: strictly consume the next token or fail with a typed error
    ctx.expect(Then, self)?;
    let then  = self.arena.alloc(ctx.sub.parse(self)?);
    
    ctx.expect(Else, self)?;
    let else_ = self.arena.alloc(ctx.sub.parse(self)?);

    Ok(Expression {
        kind: ExprKind::If { cond, then, else_ },
        span: Span::new(cond.span.start, else_.span.end),
        ty:   None,
    })
}
```

*Need to parse argument lists? `ctx.separated(Ident, Comma, RParen, self)?` does it safely.*

## 4. Bring Your Own Types, Memory, and State
`vpratt` is strictly zero-allocation and `no_std` compatible. It gets completely out of your way. 
Our examples use `Box` for convenience, but you are free to use arena allocators (like `bumpalo`), `Rc`, or any custom AST representation. 

Because every handler receives `&mut self`, you can seamlessly pass and mutate **any additional state** without relying on globals or convoluted context objects. Need a symbol table, a diagnostic accumulator, or an arena? Just put it in your parser struct!

```rust
#[vpratt::parser(
    stream  = self.stream,           // Your Peekable iterator
    output  = Expression<'a>,        // Your AST (e.g. Arena-allocated references)
    item    = Token<'a>,             // Your lexer output
    token   = TokenKind<'a>,         // Your routing type
    error   = Diagnostic<'a>,        // Your error type
)]
impl<'a> Parser<'a> { 
    // Handlers take `&mut self`, granting you full access to your custom state
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
// Map the engine's mechanical failures to your beautiful diagnostics
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
