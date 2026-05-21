## vpratt 
**Build unhinged pratt parsers**

[![Crates.io](https://img.shields.io/crates/v/vpratt.svg)](https://crates.io/crates/vpratt)
[![Docs.rs](https://docs.rs/vpratt/badge.svg)](https://docs.rs/vpratt)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

<img src="vpratt banner dark.svg" alt="">

Parsing mathematical expressions and expression-based languages is unnecessarily difficult.<br>
Parser combinator libraries struggle with left-recursion and mathematical precedence.<br>
On the other hand, writing hand-rolled recursive descent and top-down parsers, lead to massive spaghetti match statements<br>
which tastes like bugs, not good.


`vpratt` is a zero-boilerplate framework for building $\mathcal{O}(N)$ [Pratt parsers](https://en.wikipedia.org/wiki/Operator-precedence_parser) (Top-Down Operator Precedence Parsers) in Rust.<br>

## Why vpratt?

|                               | `nom` / `chumsky`    | Hand-rolled     | `vpratt`          |
|-------------------------------|----------------------|-----------------|-------------------|
| Left-recursion                | Requires workarounds | Supported       | Supported         |
| Operator precedence           | Manual and fragile   | Error-prone     | Declarative table |
| Implied multiplication (`2x`) | Needs lexer hacks    | Needs lookahead | Native            |
| Compile-time safety           | Guaranteed           | None            | Guaranteed        |
| Boilerplate                   | High                 | Very high       | Minimal           |

## Features

1. **Static Routing** — Define your grammar once in a declarative builder DSL. Precedence numbers live in exactly one place.
2. **Complete Modularity** — The `vpratt` core owns the state loop. You write isolated, bite-sized handler functions with your custom domain logic.
3. **Absolute Safety** — The macro-driven architecture injects *Capability Tokens* into your handlers at compile time, guaranteeing mathematically sound Left Binding Power (LBP) state functions. No more off-by-one errors.
4. **Native Juxtaposition** — Implied operations like `2x` or `f(x)` are handled natively in strict $\mathcal{O}(N)$ time, with no lexer hacks or lookaheads required.

> **Read the full framework reference manual [here](DOC.md)** to see the architecture that backs the state-machine guarantees.


## Installation

```toml
[dependencies]
vpratt = "0.1"
```

Or via cargo:

```sh
cargo add vpratt
```
 
---



## Quick Start
With `vpratt` you do not write the internal loops, you define your domain types, write your rules and map them to handlers. Simple!

```rust
use vpratt::{Consumed, Rhs, Table, Associativity::Left};
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
    fn num(&mut self, c: Consumed<TokenKind>) -> vpratt::Result<Self>{
        match c.token{ Num(val) => Ok(Expr::Num(val)), _ => unreachable!() }
    }

    #[vpratt::handler]
    fn add(&mut self, lhs: Expr, _op: Consumed<TokenKind>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Add(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn sub(&mut self, lhs: Expr, _op: Consumed<TokenKind>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Sub(Box::new(lhs), Box::new(rhs.parse(self)?)))
    }

    #[vpratt::handler]
    fn negate(&mut self, _op: Consumed<TokenKind>, rhs: Rhs<Self>) -> vpratt::Result<Self>{
        Ok(Expr::Neg(Box::new(rhs.parse(self)?)))
    }
}

fn main() {
    // let src = "2 + -3 - 4 + 5";
    let tokens = vec![Num(2.0), Plus, Minus, Num(3.0), Minus, Num(4.0), Plus, Num(5.0) ];
    let mut calc = Calc::new(tokens.into_iter());
    let ast = calc.pratt_parse();
    match ast {
        Ok(ast) => println!("AST:\n {:#?}", ast),
        Err(err) => eprintln!("Error: {:#?}", err)
    }
}


```

## Native Juxtaposition operations


Standard parser frameworks usually require ugly lexer hacks (like manually inserting invisible `*` tokens) or unlimited lookaheads to handle implied multiplication like `2x` or juxtaposition like `2(a + b)`.<br>

Because `vpratt` allows handlers to dynamically reinject synthetic AST nodes back into the state machine, it solves implicit math natively in strict $\mathcal{O}(N)$ time.<br>


```rust
// 1. In your table, map an adjacency binding format to your handler
.implied(30, Left, TokenKind::Ident, Self::implied_mul)

// 2. implied formats have the signature used below
#[vpratt::handler]
fn implied_mul(
    &mut Self, 
    lhs: Expr, 
    _: Consumed<Token>, 
    resume: Resume<Self> // Uses Resume to immediately re-enter the engine
) -> vpratt::Result<Self> {
    
    // We construct a multiplication node, and use our capability token 
    // to grab the right side without breaking precedence.
    let rhs = resume.parse(self)?;
    
    Ok(Expr::Mul(Box::new(lhs), Box::new(rhs)))
}
```

## Error Handling

`vpratt` was designed for production compilers and language servers. The engine surfaces exactly three mechanical failure modes as variants of `VprattError`:

| Variant                 | When it fires                                                                       |
|-------------------------|-------------------------------------------------------------------------------------|
| `UnexpectedEOF`         | Stream ran out while the engine expected an RHS (e.g. trailing `2 +`)               |
| `UnmatchedDelimiter`    | A `.group()` or `.juxt()` reached EOF without its closing delimiter (e.g. `(a + b`) |
| `UnexpectedToken`       | A token appeared with no matching prefix/terminal/group rule (e.g. `* 2`)           |
| `ExpectedTokenMismatch` | The expected token was not found                                                    |

These map cleanly onto your own diagnostic types:

```rust
impl From<VprattError> for MyDiagnostic {
    fn from(err: VprattError) -> Self {
        match err {
            VprattError::UnexpectedEOF              => MyDiagnostic::MissingExpression,
            VprattError::UnmatchedDelimiter         => MyDiagnostic::UnclosedGroup,
            VprattError::UnexpectedToken            => MyDiagnostic::InvalidToken,
            VprattError::ExpectedTokenMismatch      => MyDiagnostic::ExpectedTokenMismatch,
        }
    }
}
```
 
---

## Installation
```bash
cargo add vpratt
```
