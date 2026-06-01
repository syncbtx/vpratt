<small>## Macro Reference

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
<small>

| Argument  | Required | Description                                                                                             |
|-----------|----------|---------------------------------------------------------------------------------------------------------|
| `stream`  | ✅        | Path to the `Peekable` iterator field on `self`                                                         |
| `output`  | ✅        | The AST node type returned by handlers                                                                  |
| `item`    | ✅        | The `Iterator::Item` type of the stream                                                                 |
| `token`   | ✅        | The routing discriminant type (often a `TokenKind` enum)                                                |
| `entry`   | ☑️       | Name of the generated entry-point method. Default: `pratt_parse`                                        |
| `extract` | ☑️       | Closure or fn path to extract `token` from `item`. Default: identity clone when `item == token`         |
| `error`   | ☑️       | Custom error type. Must implement `From<VprattError<Item, Token>>`. Default: `VprattError<Item, Token>` |
</small>

---

#### `#[vpratt::token(...)]`

Apply this to your token type — a struct, enum, or type alias. Tells vpratt how to extract the routing token from a stream item, and optionally where the span is.

```rust
// item is its own routing token
#[vpratt::token(self)]
pub enum TokenKind { ... }

// item wraps the routing token in a named field
#[vpratt::token(kind, span)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
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
    item    = Token<'a>,
    token   = TokenKind<'a>,
    output  = Expr,
    extract = |t: &Token<'a>| t.kind.clone(),
)]

// with #[vpratt::token(kind)] — token and extract derived
#[vpratt::parser(
    stream = self.stream,
    item   = Token<'a>,
    output = Expr,
)]
```

The second form is shorter but both are valid. Use whichever fits your project structure — `#[vpratt::token]` is purely opt-in.

---

##### `#[vpratt::handler]`

Marker on handler functions inside a `#[vpratt::parser]` impl block. Validates the handler signature against the format it was registered with in the table.
</small>