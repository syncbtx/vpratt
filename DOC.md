# vpratt — Reference Manual

To use `vpratt`, you do not write a parsing loop or gigantic match statements.<br>
You only need to understand how these four components interact to hand control back and forth between `vpratt` and your code:
> 1. **Formats** — table entries that declare what a token *is* (terminal, infix, prefix, …)
> 2. **Handlers** — user-defined functions that receive a specific capability context from the engine and use it to construct and return a node for your Abstract Syntax Tree (AST). You can execute virtually any domain logic here, and multiple formats can route to the same handler.
> 3. **Capability Tokens** — zero-cost Rust structs that act as strict, compile-time permissions, guaranteeing that a handler function only executes operations mathematically valid for its specific position in the parsing state machine.
> 4. **CtxUtils** — safe, typed stream navigation for structural and keyword-driven syntax



---

## The Table

`vpratt` uses a declarative, builder-based DSL to define your grammar.<br>
If you've ever looked at a precedence table online, writing a `vpratt` table will feel exactly like that.<br>
While an entry in the table locks the required signature, you can map multiple formats to a single handler function.

There are exactly two slots for any given token: **nud** (Null Denotation) and **led** (Left Denotation), mapping conceptually to the **prefix** and **infix** phases. A token can appear in each slot at most once. For example, a `Minus` token can be registered in the prefix slot as negation (`-x`) and in the infix slot as subtraction (`x - y`) simultaneously.

```rust
const TABLE: Table<Self> = Table::new()
    // ── Prefix phase: tokens that START an expression ─────────
    .terminal(token, handler)                       // atomic values
    .group(open, close, handler)                    // enclosed sub-expressions
    .prefix(bp, token, handler)                     // unary prefix operators
    .structural(token, handler)                 // keyword constructs

    // ── Infix phase: tokens that CONTINUE an expression ───────
    .infix(bp, assoc, token, handler)               // binary operators
    .postfix(bp, token, handler)                    // trailing operators
    .implied(bp, assoc, token, handler)             // adjacency (no operator token)
    .juxt(bp, open, close, handler);                // adjacency into a group
```

`bp` is the binding power (`u16`). Higher binds tighter. `assoc` is `Left` or `Right`.

---

## Formats & Contexts

Each table entry (a *format*) locks the handler to a specific context type. The context contains only the capabilities that format requires.

### Prefix Phase

#### `.terminal(token, handler)`

Standalone atomic values: numbers, identifiers, booleans.

```rust
fn handler(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.consumed` | `Consumed<Item>` | The token that was consumed |

```rust
#[vpratt::handler]
fn num(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self> {
    let val = ctx.consumed.token.lexeme.parse::<f64>().unwrap();
    Ok(Expr::Num(val))
}
```

---

#### `.group(open, close, handler)`

Enclosed sub-expressions: `(a + b)`, `[items]`, `{block}`.

```rust
fn handler(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.consumed` | `Consumed<Item>` | The opening delimiter token |
| `ctx.enclosed` | `Enclosed<P>` | Parses interior at bp 0, then consumes the closing delimiter |

```rust
#[vpratt::handler]
fn grouped(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self> {
    let (inner, close_token) = ctx.enclosed.parse(self)?;
    Ok(Expr::Group(Box::new(inner)))
}
```

`enclosed.parse()` returns `(Output, Item)` — the parsed expression and the raw closing token.

---

#### `.prefix(bp, token, handler)`

Unary prefix operators: `-x`, `!flag`, `~bits`.

```rust
fn handler(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.consumed` | `Consumed<Item>` | The prefix operator token |
| `ctx.rhs` | `Rhs<P>` | Parses the right-hand side at this operator's binding power |

```rust
#[vpratt::handler]
fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
}
```

---

#### `.structural(token, handler)`

The escape hatch from the math engine. Keyword-driven constructs like `let`, `if`, `match`, `while` that don't follow precedence rules.

```rust
fn handler(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.consumed` | `Consumed<Item>` | The keyword token |
| `ctx.atom` | `Atom<P>` | Consumes and routes a single prefix token without entering the infix loop |
| `ctx.sub` | `Subexpr<P>` | Parses a complete expression at bp 0 (fresh context) |

```rust
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

### Infix Phase

#### `.infix(bp, assoc, token, handler)`

Standard binary operators: `+`, `-`, `*`, `/`, `^`, `<-`.

```rust
fn handler(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.lhs` | `Output` | The already-parsed left-hand side |
| `ctx.consumed` | `Consumed<Item>` | The operator token |
| `ctx.rhs` | `Rhs<P>` | Parses the right-hand side at the correct binding power |

Associativity is baked into the `Rhs` token automatically:
- `Left`  → `rhs` uses `bp` as its threshold (same-precedence ops bind left)
- `Right` → `rhs` uses `bp - 1` (same-precedence ops bind right)

```rust
#[vpratt::handler]
fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
}
```

---

#### `.postfix(bp, token, handler)`

Trailing operators: `x!`, `i++`, `val?`.

```rust
fn handler(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.lhs` | `Output` | The expression being postfixed |
| `ctx.consumed` | `Consumed<Item>` | The postfix operator token |

No further parsing capabilities — the expression terminates here.

```rust
#[vpratt::handler]
fn factorial(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self> {
    Ok(Expr::Fact(Box::new(ctx.lhs)))
}
```

---

#### `.implied(bp, assoc, token, handler)`

Adjacency binding with no operator token: `2x`, `f a b`, `sin x`.

The engine has already consumed the adjacent token — you get it in `consumed`. Use `seed` to feed it back into the prefix router, then `resume` to let trailing operators bind before collapsing.

```rust
fn handler(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.lhs` | `Output` | The left expression |
| `ctx.consumed` | `Consumed<Item>` | The adjacent token (already consumed) |
| `ctx.seed` | `Seed<P>` | Feeds the consumed token back into the prefix router |
| `ctx.resume` | `Resume<P>` | Re-enters the infix loop with a synthetic `lhs` |

```rust
// Parsing: 2x^3  →  Mul(2, Pow(x, 3))
#[vpratt::handler]
fn implied_mul(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
    let arg  = ctx.seed.parse(self, ctx.consumed.token)?;   // x → Var("x")
    let full = ctx.resume.parse(self, arg)?;                // Var("x") + ^3 → Pow(x, 3)
    Ok(Expr::Mul(Box::new(ctx.lhs), Box::new(full)))        // 2 * Pow(x, 3)
}
```

---

#### `.juxt(bp, open, close, handler)`

Adjacency binding into an enclosed group: `2(a + b)`, `f(x, y)`.

```rust
fn handler(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self>
```

| Field | Type | Description |
|-------|------|-------------|
| `ctx.lhs` | `Output` | The left expression |
| `ctx.consumed` | `Consumed<Item>` | The opening delimiter token |
| `ctx.enclosed` | `Enclosed<P>` | Parses interior at bp 0, consumes closing delimiter |
| `ctx.resume` | `Resume<P>` | Re-enters the infix loop with a synthetic `lhs` |

```rust
// Parsing: 2(a + b)^2  →  Pow(Mul(2, Add(a, b)), 2)
#[vpratt::handler]
fn implicit_mul(&mut self, ctx: JuxtCtx<Self>) -> vpratt::Result<Self> {
    let (inner, _) = ctx.enclosed.parse(self)?;
    let node = Expr::Mul(Box::new(ctx.lhs), Box::new(inner));
    ctx.resume.parse(self, node)    // let ^2 bind to the multiplication result
}
```

---

## Capability Tokens

Every capability token encapsulates a single state transition. You cannot misuse them — they carry the exact binding power or delimiter information needed, injected by the macro at construction time.

| Token            | What it does                                                  | Signature                                  |
|------------------|---------------------------------------------------------------|--------------------------------------------|
| `Consumed<Item>` | Holds the raw token that triggered the handler.               | `consumed.token`                           |
| `Atom<P>`        | Consumes and routes one prefix token (no infix loop).         | `atom.parse(self)?` → `Output`             |
| `Rhs<P>`         | Parses the right-hand side at the operator's binding power.   | `rhs.parse(self)?` → `Output`              |
| `Subexpr<P>`     | Parses a fresh expression at bp 0.                            | `sub.parse(self)?` → `Output`              |
| `Enclosed<P>`    | Parses interior at bp 0, then consumes the closing delimiter. | `enclosed.parse(self)?` → `(Output, Item)` |
| `Seed<P>`        | Feeds a token back into the prefix router.                    | `seed.parse(self, token)?` → `Output`      |
| `Resume<P>`      | Re-enters the infix loop with a synthetic `lhs`.              | `resume.parse(self, lhs)?` → `Output`      |

---

## CtxUtils

The `CtxUtils` trait is implemented for **every** context type. These methods extend the engine's safety guarantees into structural, non-precedence-driven parsing.

All methods take `&self` (the context) and `p: &mut P` (the parser).

### `expect`

Consume the next token if it matches, otherwise fail with `ExpectedTokenMismatch`.

```rust
ctx.expect(self, Equals)?;         // → Consumed<Item>
```

### `accept`

Optionally consume the next token if it matches. Never fails.

```rust
let is_mut = ctx.accept(self, Mut)?.is_some();   // → Option<Consumed<Item>>
```

### `series`

Consume a sequence of matching tokens until a delimiter is reached. The delimiter is **not** consumed.

```rust
// stream: [Ident("x"), Ident("y"), Ident("z"), Equals, ...]
let params = ctx.series(self, Ident, Equals)?;
// params: [Consumed("x"), Consumed("y"), Consumed("z")]
// stream: [Equals, ...]   ← delimiter left in place
```

### `separated`

Consume tokens separated by `sep` until `end` is reached. The end delimiter **is** consumed.

```rust
// stream: [Ident("x"), Comma, Ident("y"), Comma, Ident("z"), RParen, ...]
let args = ctx.separated(self, Ident, Comma, RParen)?;
// args: [Consumed("x"), Consumed("y"), Consumed("z")]
// stream: [...]   ← RParen consumed
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

### `#[vpratt::parser(...)]`

Applied to an `impl` block. Reads the `TABLE` const and generates the full Pratt state machine.

| Argument  | Required | Description                                                                                             |
|-----------|----------|---------------------------------------------------------------------------------------------------------|
| `stream`  | ✅        | Path to the `Peekable` iterator field on `self`                                                         |
| `output`  | ✅        | The AST node type returned by handlers                                                                  |
| `item`    | ✅        | The `Iterator::Item` type of the stream                                                                 |
| `token`   | ✅        | The routing discriminant type (often a `TokenKind` enum)                                                |
| `entry`   | ☑️       | Name of the generated entry-point method. Default: `pratt_parse`                                        |
| `extract` | ☑️       | Closure or fn path to extract `token` from `item`. Default: identity clone when `item == token`         |
| `error`   | ☑️       | Custom error type. Must implement `From<VprattError<Item, Token>>`. Default: `VprattError<Item, Token>` |

### `#[vpratt::handler]`

Marker on handler functions inside a `#[vpratt::parser]` impl block. Validates the handler signature against the format it was registered with in the table.
