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
use core::ops::Range;
use core::iter::Peekable;
use vpratt::{Table, Consumed, Rhs, VprattError};
use TokenKind::*;

// Domain routing type: used by the engine to navigate precedence
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Num(f64),
    Plus, Minus, Star, Slash
}

// Rich token type
#[derive(Debug)]
pub struct Token {
    kind: TokenKind,
    span: Range<usize>
}

// Domain AST type
#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>), Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>), Div(Box<Expr>, Box<Expr>),
}

// Dummy struct to hold the stream of tokens.
// Note: you can pass additional state to your handlers via `self`
#[derive(Debug)]
pub struct Calc<I: Iterator<Item = Token>> {
    stream: Peekable<I>
}

#[vpratt::parser(
    stream = self.stream,                   // the stream is expected to be a peekable iterator
    entry  = parse_calc,                   // custom entry
    token  = TokenKind,                   // defines the routing token type     
    extract = |t: &Token| t.kind.clone() // tells the core how to extract the routing token from the Iterator's Item.
)]
impl<I: Iterator<Item = Token>> Calc<I> {
    pub fn new(stream: I) -> Self { Self { stream: stream.peekable() } }

    const TABLE: Table<Self> = Table::new()
        .terminal(Num(0.0),     Self::num)
        
        .infix( 10, Left, Plus,  Self::additive)
        .infix( 10, Left, Minus, Self::additive)
        .infix( 20, Left, Star,  Self::multiplicative)
        .infix( 20, Left, Slash, Self::multiplicative)
        .prefix(30, Minus, Self::negate); 

    #[vpratt::handler] // marks a handler and provides trace features for debugging
    fn num(_: &mut Self, consumed: Consumed<Token>) -> Result<Expr, VprattError> {
        let Num(n) = consumed.token.kind else { unreachable!() };
        Ok(Expr::Num(n))
    }
    
    #[vpratt::handler]
    fn negate(p: &mut Self, _: Consumed<Token>,  rhs: Rhs<Self>) -> Result<Expr, VprattError> {
        Ok(Expr::Neg(Box::new(rhs,parse(p)?)))
    }

    #[vpratt::handler] 
    fn additive(p: &mut Self, lhs: Expr, op: Consumed<Token>, rhs: Rhs<Self>) -> Result<Expr, VprattError> {
        match op.token.kind {
            Plus => Ok(Expr::Add(Box::new(lhs), Box::new(rhs.parse(p)?))),
            Minus => Ok(Expr::Sub(Box::new(lhs), Box::new(rhs.parse(p)?))),
            _ => unreachable!()
        }
    }

    #[vpratt::handler]
    fn multiplicative(p: &mut Self, lhs: Expr, op: Consumed<Token>, rhs: Rhs<Self>) -> Result<Expr, VprattError> {
        match op.token.kind {
            Star => Ok(Expr::Mul(Box::new(lhs), Box::new(rhs.parse(p)?))),
            Slash => Ok(Expr::Div(Box::new(lhs), Box::new(rhs.parse(p)?))),
            _ => unreachable!()
        }
    }
}

fn main() -> Result<(), VprattError> {
    let src = "2 + 3 * 4";
    let tokens: Vec<Token> = lexer.tokenize(src); // Assuming a hypothetical lexer is used
    let parser = Calc::new(tokens.into_iter());
    let ast = parser.parse_calc()?;

    println!("{:#?}", ast);
    Ok(())
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
    p: &mut Self, 
    lhs: Expr, 
    _: Consumed<Token>, 
    resume: Resume<Self> // Uses Resume to immediately re-enter the engine
) -> Result<Expr, VprattError> {
    
    // We construct a multiplication node, and use our capability token 
    // to grab the right side without breaking precedence.
    let rhs = resume.parse(p)?;
    
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
