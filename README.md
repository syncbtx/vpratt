## vpratt 
**Mathematically correct Pratt parsers, effortlessly.**

<img src="vpratt banner dark.svg">

Parsing mathematical expressions and expression-based languages is unnecessarily difficult.<br>
Parser combinator libraries struggle with left-recursion and mathematical precedence.<br>
On the other hand, writing hand-rolled recursive descent and top-down parsers, lead to massive spaghetti match statements<br>
which tastes like bugs, not good.


`vpratt` is a zero-boilerplate framework for building $\mathcal{O}(N)$ Pratt parsers(Top-Down Operator Precedence Parsers) in `Rust`<br>

It provides:<br>
1. **Static Routing:** You define your grammar once in a declarative builder DSL. This is the only time you deal with numbers.
2. **Complete modularity:** The `vpratt` core owns and controls the state loop. You write isolated, bite-sized handler functions with your custom domain logic.
3. **Absolute Safety:** The macro-driven architecture injects `Capability Tokens` into your handlers at compile time, guaranteeing mathematically sound Left Binding Power(LBP) state functions. No more off-by-one errors!

> **Read the full framework reference manual [here](DOC.md)** to see the architecture that backs the state-machine guarantees.

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

## Diagnostics & Error Handling
vpratt was designed for production compilers and language servers.

If the engine encounters a mathematically invalid state, it yields a `VprattError` enum variant.<br>
`vpratt` has exactly 3 mechanical failure modes:<br>

1. Unexpected EOF: The stream ran out of tokens while the engine was actively waiting for the right-hand side of an expression (e.g., a trailing 2 + ).<br>

2. Unmatched Delimiter: An enclosed format (like .group() or .juxt()) reached the end of the stream without encountering its required closing delimiter (e.g., (a + b).<br>

3. Unexpected Token: The engine expected an expression to begin, but the current token has no .prefix(), .terminal(), or .group()<br>
rule associated with it (e.g., encountering a * at the start of a statement).<br>

```rust
impl From<VprattError> for MyCustomError {
    fn from(err: VprattError) -> Self {
        match err {
            VprattError::UnexpectedEOF => MyCustomError::MissingExpression,
            // ... map mechanical errors to your beautiful, spanned diagnostics
        }
    }
}
```

## Installation
```bash
cargo add vpratt
```
