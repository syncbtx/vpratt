# <mark style="background-color:#012a36">`vpratt`</mark>
<small>

## Reference Manual

### How vpratt works

A `Minus` token means two different things depending on where it appears in the stream. In a <mark style="background-color:#29274c">`prefix`</mark>
position it is negation — `-x`. In an <mark style="background-color:#31241F">`infix`</mark> position it is subtraction — `x - y`. Same token, two completely different operations. The parser needs to know which is which at every point in the stream.

That distinction is the entire foundation of Pratt parsing, and it is the foundation of vpratt as well.   
Every decision in the framework traces back to one question: *which position does this token appear in?*  
That question drives every decision you will ever make when working with vpratt — which format to register a token under, 
which capability tokens your handler receives, which phase of the engine fires. Once you have that instinct, everything else just flows.

---

### Definitions

**`Handlers`** are functions you write, each specific to your problem domain — each handler receives the consumed token that triggered it and whatever context it needs, constructs a node, and returns it.

**`Formats`** are the declarations that connect tokens to handlers. Every token in your grammar appears in one of two positions — <mark style="background-color:#29274c">`prefix`</mark> or <mark style="background-color:#31241F">`infix`</mark> — and behaves differently in each. vpratt has eight formats in total — four prefix, four infix — each one defines a distinct contract that determines exactly what context a handler receives. That contract is enforced through capability tokens.

**`Capability tokens`** are how the engine hands off control to your handlers, safely. 
Each one represents a specific operation valid at that position in the parse — parsing a right-hand side, consuming an enclosed group, re-entering the infix loop. 
If an operation is not valid at that position, the handler will not receive that capability.

**`CtxUtils`** cover the parts of your grammar that do not follow precedence rules — keywords, bindings, argument lists. `expect`, `accept`, `series`, `separated`, `series_sub`, `separated_sub`.   
These let you interact with the stream safely without writing loops or peeking manually.

---

To see how fundamental the two-position idea is, consider `2x` — implicit multiplication with no operator token.  
`2` opens the expression in <mark style="background-color:#29274c">`prefix`</mark> position as usual. But `x` — a plain identifier — appears in <mark style="background-color:#31241F">`infix`</mark> position with nothing before it. No `*`, no explicit operator, just adjacency.

Because `x` is registered as `.implied()`, the engine recognises it in infix position and fires the implied handler with `x` as the consumed token. From there, `x` is seeded into the internal prefix table to produce the actual AST node. Once we have the `x` node we feed it into the infix loop using [`resume`](crates/vpratt/src/capabilities.rs) to allow any trailing operator that outranks the `x`'s infix binding power — a `^3` for instance — to bind. Only then does the multiplication collapse.

```rust
.implied(80, Left, Ident(""), Self::implied_mul)

#[vpratt::handler]
fn implied_mul(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
    let rhs = ctx.seed.parse(self, ctx.consumed.token)?;
    let rhs = ctx.resume.parse(self, rhs)?;
    Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(rhs)))
}
```

<hr style="height:2px; border-width:0; background-color:gray; background-image:linear-gradient(to right, rgba(0,0,0,0), rgba(0,0,0,0.75), rgba(0,0,0,0));">

## The Table

`vpratt` uses a declarative, builder-based DSL to define your grammar.  
Each builder method defines a format.  
The function pointers in the table are type-checked by rustc against the format they are declared under. 
A handler with the wrong signature is a compile error. If the table compiles, the routing is sound.

The advantage of this approach is that you as a programmer can trust the table to be syntactically correct if it compiles, if it fails or the output is incorrect you only debug the table which you can do by simply visually scanning the table in a single pass.  


```rust
const TABLE: Table<Self> = Table::new()
    // Prefix: tokens that START an expression 
    .terminal(token, handler)                       // atomic values
    .group(open, close, handler)                    // enclosed sub-expressions
    .prefix(bp, token, handler)                     // unary prefix operators
    .structural(token, handler)                     // keyword constructs

    // Infix: tokens that CONTINUE an expression 
    .infix(bp, assoc, token, handler)               // binary operators
    .postfix(bp, token, handler)                    // trailing operators
    .implied(bp, assoc, token, handler)             // adjacency (no operator token)
    .juxt(bp, open, close, handler);                // adjacency into a group
```

`bp` is the binding power (`u16`) of a token. Higher binds tighter.  
`assoc` can either be Left or Right and it defines how to break the tie between tokens with the same binding power:   
`Left: ((a - b) - c)`, `Right: (a ^ (b ^ c))`.

```rust
const TABLE: Table<Self> = Table::new()
    .terminal(Num(0.0),         Self::literal)
    .terminal(Ident(""),        Self::literal)
    .group(LParen, RParen,      Self::group)
    
    .infix(  20, Left,  Plus,   Self::add)
    .infix(  20, Left,  Minus,  Self::sub)
    .infix(  30, Left,  Star,   Self::mul)
    .infix(  30, Left,  Slash,  Self::div)
    .prefix( 40,        Minus,  Self::negate)
    .infix(  50, Right, Caret,  Self::pow)
    .postfix(60,        Bang,   Self::fact)
;
```



---

## Formats
Each format is a declaration that connects a token in a specific affix position to a handler.   
The format you choose determines what capabilities that handler receives — no more, no less.

---

### Prefix Formats


#### `.terminal(token, handler)`

The simplest format. Atomic values that start an expression and need nothing else — numbers, identifiers, booleans.   
No recursion, no right-hand side. The handler receives the consumed token and returns a node.

```rust
const TABLE: Table<Self> = Table::new()
    .terminal(Int(0),         Self::literal)
    .terminal(Float(0.0),     Self::literal)
    .terminal(Ident(""),      Self::literal)
    .terminal(True),          Self::literal)
    .terminal(False),         Self::literal);

#[vpratt::handler]
fn literal(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self> {
    match ctx.consumed.token.kind {
        TokenKind::Int(n)   => Ok(Expr::Int(n)),
        TokenKind::Float(f) => Ok(Expr::Float(f)),
        TokenKind::Ident(s) => Ok(Expr::Var(s)),
        TokenKind::True     => Ok(Expr::True),
        TokenKind::False    => Ok(Expr::False),
        _ => unreachable!()
    }
}
```

Multiple tokens can route to the same handler if they are declared in the same format. All five literals above — Int, Float, Ident, True, False — share one function.

---

#### `.group(open, close, handler)`

Enclosed sub-expressions: `(a + b)`, `[items]`, `{block}`.   
The engine consumes the opening delimiter and hands the handler an Enclosed capability that parses the interior and consumes the closing delimiter.   
The closing token is returned alongside the inner expression so your handler has access to its span.  

```rust
const TABLE: Table<Self> = Table::new()
    .group(LParen,RParen, Self::group);

#[vpratt::handler]
fn group(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self> {
    let (inner, _close) = ctx.enclosed.parse(self)?;
    Ok(inner)
}
```

*The bracket pair declared here — open and close — is also used by .juxt() when the same brackets appear in infix position.*

| Field | Type | Description |
|-------|------|-------------|
| `ctx.consumed` | `Consumed<Item>` | The opening delimiter token |
| `ctx.enclosed` | `Enclosed<P>` | Parses interior at bp 0, then consumes the closing delimiter |

---

#### `.prefix(bp, token, handler)`

Unary prefix operators: -x, !flag, ~bits.  
The handler receives the consumed operator and an Rhs capability to parse the operand. 
Associativity for prefix is always right — `--x` parses as `-(-(x))`.

```rust
const TABLE: Table<Self> = Table::new()
    .prefix( 40,        Minus,  Self::negate);

#[vpratt::handler]
fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
}
```

---

#### `.structural(token, handler)`

The bridge between Pratt and recursive descent.  
Keywords like `let`, `if`, `while` do not follow precedence rules — they are structural.  
The handler receives the `Atom` capability to consume single tokens without entering the infix loop, and the `Subexpr` capability to parse complete expressions from bp 0.
```rust
const TABLE: Table<Self> = Table::new()
    .structural(If, Self::parse_if);

#[vpratt::handler]
fn parse_if(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
    let start = ctx.consumed.token.span.start;

    let cond = ctx.sub.parse(self)?;
    ctx.expect(self, Then)?;
    let then = ctx.sub.parse(self)?;
    ctx.expect(self, Else)?;
    let else_ = ctx.sub.parse(self)?;

    let end = else_.span.end;
    Ok(Expression {
        kind: ExprKind::If(self.arena.alloc(IfNode {
            cond:      self.arena.alloc(cond),
            then_expr: self.arena.alloc(then),
            else_expr: self.arena.alloc(else_),
        })),
        span: Span::new(start, end),
        ty: None,
    })
}
```

```rust
const TABLE: Table<Self> = Table::new()
    .structural(Let, Self::parse_let);

#[vpratt::handler]
fn parse_let(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
    let name  = ctx.expect(self, Ident)?;              // CtxUtils
    let args  = ctx.series(self, Ident, Equals)?;      // CtxUtils
    ctx.expect(self, Equals)?;
    let value = ctx.sub.parse(self)?;                   // Subexpr capability
    ctx.expect(self, In)?;
    let body  = ctx.sub.parse(self)?;
    Ok(Expr::Let { name, args, value: Box::new(value), body: Box::new(body) })
}
```


---

### Infix Formats

#### `.infix(bp, assoc, token, handler)`

Standard binary operators. The handler receives the already-parsed left-hand side, the consumed operator, and an Rhs capability to parse the right-hand side.  
Associativity is baked into Rhs automatically — there is nothing to configure in the handler body.

```rust
const TABLE: Table<Self> = Table::new()
    .infix(20, Left, Plus, Self::add);
    
#[vpratt::handler]
fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
}
```

When multiple operators share a handler, ctx.consumed tells you which one fired:

```rust
const TABLE: Table<Self> = Table::new()
    .infix(10, Left, Less,    Self::cmp)
    .infix(10, Left, Greater, Self::cmp)
    .infix(10, Left, LessEq,  Self::cmp)
;

#[vpratt::handler]
fn cmp(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
    let op = match ctx.consumed.token.kind {
        TokenKind::Less    => CmpOp::Lt,
        TokenKind::Greater => CmpOp::Gt,
        TokenKind::LessEq  => CmpOp::LtEq,
        _ => unreachable!()
    };
    Ok(Expr::Cmp(op, Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
}
```

---

#### `.postfix(bp, token, handler)`

Trailing operators: x!, val?. The handler receives the left-hand side and the consumed operator. There is no Rhs — the expression terminates here.

```rust
const TABLE: Table<Self> = Table::new()
    .infix(80, Bang,  Self::factorial);

#[vpratt::handler]
fn factorial(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Fact(Box::new(ctx.lhs)))
}
```

No further parsing capabilities — the expression terminates here.

---

#### `.implied(bp, assoc, token, handler)`

Adjacency with no operator token — 2x, f a, sin θ. Covered in depth in the opening.   
The short version: the adjacent token is already consumed when the handler fires. seed routes it back through the prefix table. resume lets tighter operators bind before the implicit operation collapses.

```rust
const TABLE: Table<Self> = Table::new()
    .implied(30, Left, Ident(""),  Self::implied_mul);

// Parsing: 2x^3  →  Mul(2, Pow(x, 3))
#[vpratt::handler]
fn implied_mul(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
    let arg  = ctx.seed.parse(self, ctx.consumed.token)?;   
    let full = ctx.resume.parse(self, arg)?;                
    Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(full)))       
}
```

---

#### `.juxt(bp, open, close, handler)`

Adjacency into an enclosed group — 2(a + b), f(x, y). The opening delimiter fires the handler in infix position. Enclosed parses the interior and consumes the closing delimiter. 
Resume lets trailing operators bind before the result collapses.

```rust
const TABLE: Table<Self> = Table::new()
    .juxt(30, LParen, RParen,  Self::implicit_mul);

// Parsing: 2(a + b)^2  →  Pow(Mul(2, Add(a, b)), 2)
#[vpratt::handler]
fn implicit_mul(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self> {
    let (inner, _) = ctx.enclosed.parse(self)?;
    let node = ctx.resume.parse(self, inner)?;    
    Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(node)))
}
```


---

## Capability Tokens

Capability tokens are the interface between the engine and your handlers. 
Each one is a zero-cost Rust struct carrying exactly the state it needs — binding power, delimiter predicate, router reference. 
They are constructed by the macro at compile time and injected by the engine at runtime. You cannot construct them yourself, and you cannot receive one your format does not permit.

| Token            | What it does                                                  | Signature                                  |
|------------------|---------------------------------------------------------------|--------------------------------------------|
| `Consumed<Item>` | Holds the raw token that triggered the handler.               | `consumed.token`                           |
| `Atom<P>`        | Consumes and routes one prefix token (no infix loop).         | `atom.parse(self)?` → `Output`             |
| `Rhs<P>`         | Parses the right-hand side at the operator's binding power.   | `rhs.parse(self)?` → `Output`              |
| `Subexpr<P>`     | Parses a fresh expression at bp 0.                            | `sub.parse(self)?` → `Output`              |
| `Enclosed<P>`    | Parses interior at bp 0, then consumes the closing delimiter. | `enclosed.parse(self)?` → `(Output, Item)` |
| `Seed<P>`        | Feeds a token back into the prefix table.                     | `seed.parse(self, token)?` → `Output`      |
| `Resume<P>`      | Re-enters the infix loop with a synthetic `lhs`.              | `resume.parse(self, lhs)?` → `Output`      |

---

## CtxUtils

The `CtxUtils` trait is implemented for **every** context type. These methods extend the engine's safety guarantees into structural, non-precedence-driven parsing.

All methods take `&self` (the context) and `p: &mut P` (the parser).

### `expect`

Consume the next token if it matches, otherwise fail with `ExpectedTokenMismatch`.

```rust
// yields Consumed{token = Equals}
ctx.expect(self, Equals)?;
```

### `accept`

Optionally consume the next token if it matches. Never fails.

```rust
let is_mut = ctx.accept(self, Mut)?.is_some();   // → Option<Consumed<Item>>
```

### `series`

Collect a sequence of atomic expressions of matching `of` until `delimiter` is seen.. The delimiter is **not** consumed.  
Returns an empty `Vec` if the stream immediately contains the delimiter or a non-matching token.

```rust
 // stream: [Ident("x"), Ident("y"), Ident("z"), Equals, ...]

 let args = ctx.series(self, Ident(""), Equals)?;
 // args:   [Expr::Ident("x"), Expr::Ident::("y"), Expr::Ident("z")]
 // stream: [Equals, ...]
```

### `separated`

Collect atomic expressions of tokens matching `of`, separated by `sep`, until `end` is consumed.

```rust
// stream: [Ident("x"), Comma, Ident("y"), Comma, Ident("z"), RParen, ...]

let idents = ctx.separated(self, Ident(""), Comma, RParen)?;
// idents: [Expr::Ident("x"), Expr::Ident("y"), Expr::Ident("z")]
// stream: [...]  — RParen consumed
```

### `series_sub`

Collect a sequence of sub-expressions of matching `of` until `delimiter` is seen.. The delimiter is **not** consumed.  
Returns an empty `Vec` if the stream immediately contains the delimiter or a non-matching token.

```rust
// stream: [Ident("x"),Ident("y"), Ident("z"), Equals, ...]

let args = ctx.series_sub(self, Equals)?;
// args:   [Expr::Ident("x"), Expr::Ident("y"),Expr::Ident("z"),]
// stream: [Equals,...]
```

### `separated_sub`

Collect sub-expressions of tokens matching `of`, separated by `sep`, until `end` is consumed.

```rust
// stream: [Ident("a"), Plus, Int(1), Comma, Ident("f"), Ident("x"), Comma, ...]

let tuple = ctx.separated_sub(self, Comma, RParen)?;
// tuple:   [Expr::Add(a, 1), Expr::Application(f, x), Expr::Mul(b, 2)]
// stream: [...]
```

---

## Errors

`vpratt` surfaces exactly four mechanical failure modes:

| Variant                                   | When it fires                                                                |
|-------------------------------------------|------------------------------------------------------------------------------|
| `UnexpectedEOF`                           | Stream exhausted while the engine expected more tokens                       |
| `UnexpectedToken(Item)`                   | A token appeared with no matching prefix/terminal/group rule                 |
| `UnmatchedDelimiter(PrattToken, Item)`    | `Enclosed` reached a token that doesn't match the expected closing delimiter |
| `ExpectedTokenMismatch(PrattToken, Item)` | `expect()` found a token that doesn't match                                  |

All variants carry the offending token (or the expected one) so you can map them to spans:

```rust
impl From<VprattError<Token, TokenKind>> for MyDiagnostic {
    fn from(err: VprattError<Token, TokenKind>) -> Self {
        match err {
            VprattError::UnexpectedEOF              => MyDiagnostic::eof(),
            VprattError::UnexpectedToken(t)         => MyDiagnostic::unexpected(t.span),
            VprattError::UnmatchedDelimiter(_, t)   => MyDiagnostic::unmatched(t.span),
            VprattError::ExpectedTokenMismatch(e,t) => MyDiagnostic::mismatch(e, t.span),
        }
    }
}
```

---

## Macro Reference

##### `#[vpratt::parser(...)]`

This macro reads the `TABLE` const and generates the full Pratt state machine at compile time.  
Applied to an `impl` block of the parser struct you define.
The only required field in that struct is the stream field that carries the peekable iterator of your tokens.  
>Note: The Table must be declared in the impl to which the `#[vpratt::parser]` attribute is applied.

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
    const TABLE: Table<Self> = Table::new();
        ...
}
```

| Argument  | Required | Description                                                                                             |
|-----------|----------|---------------------------------------------------------------------------------------------------------|
| `stream`  | ✅        | Path to the `Peekable` iterator field on `self`                                                         |
| `output`  | ✅        | The AST node type returned by handlers                                                                  |
| `item`    | ✅        | The `Iterator::Item` type of the stream                                                                 |
| `token`   | ✅        | The routing discriminant type (often a `TokenKind` enum)                                                |
| `entry`   | ☑️       | Name of the generated entry-point method. Default: `pratt_parse`                                        |
| `extract` | ☑️       | Closure or fn path to extract `token` from `item`. Default: identity clone when `item == token`         |
| `error`   | ☑️       | Custom error type. Must implement `From<VprattError<Item, Token>>`. Default: `VprattError<Item, Token>` |

---

#### `#[vpratt::token(...)]`

Applied to your token type — a struct, enum, or type alias. Tells vpratt how to extract the routing token from a stream item, and optionally where the span is.

> Note: this only works if both the Item and PrattToken types don't have explicit lifetimes.

```rust
// item is its own routing token
#[vpratt::token(self)]
pub enum TokenKind { ... }

// item wraps the routing token in a named field
#[vpratt::token(kind, span)]
pub struct Token{
    pub kind: TokenKind,
    pub span: Span,
}

// item is a logos tuple — routing token at index 0
#[vpratt::token(0, 1)]
pub type LogosToken = (TokenKind, Span);
```
When applied, token and extract can be omitted from #[vpratt::parser]:

```rust 
// without #[vpratt::token] — explicit
#[vpratt::parser(
    stream  = self.stream,
    item    = Token,
    token   = TokenKind,
    output  = Expr,
    extract = |t: &Token| t.kind.clone(),
)]

// with #[vpratt::token(kind)] — token and extract derived
#[vpratt::parser(
    stream = self.stream,
    item   = Token,
    output = Expr,
)]
```

The second form is shorter but both are valid. Use whichever fits your project structure — `#[vpratt::token]` is purely opt-in.

---

##### `#[vpratt::handler]`

Marker on handler functions inside a `#[vpratt::parser]` impl block. Validates the handler signature against the format it was registered with in the table.
</small>
