## vpratt-macros

**Procedural macros for the vpratt parsing framework.**

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