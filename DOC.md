## vpratt

To use `vpratt`, you do not write a parsing loop or gigantic match statements.<br>
You only need to understand how these three components interact to hand control back and forth between `vpratt` and your code:<br>
> 1. **Formats**
> 2. **Handlers**
> 3. **Capability Tokens**.

### 1. The Routing DSL
`vpratt` uses a declarative, builder-based DSL to define your precedence rules and grammar. <br>
If you've ever looked at a precedence table online, writing a `vpratt` table will feel exactly like that.<br>
An entry in the table i.e. a builder function, maps to exactly one handler signature.

Under the hood, the core only cares about two phases of execution.<br>
1. Prefix Phase
2. Infix Phase

You map your tokens by telling the builder which phase they belong to: do they *initiate* an expression, or do they *continue* one?

#### a. Starting an Expression (The Prefix Phase)
The parser has nothing in its hands. It is looking for a token that begins a brand-new expression.

| Builder Method    | What it parses                                                     | Example               | What your Handler receives |
|:------------------|:-------------------------------------------------------------------|:----------------------|:---------------------------|
| **`.terminal()`** | A standalone, atomic value.                                        | `5`, `true`, `x`      | `Consumed`                 |
| **`.group()`**    | An enclosed sub-expression.<br>Temporarily resets precedence to 0. | `(a + b)`,<br>`{ 5 }` | `Consumed`, `Enclosed`     |
| **`.prefix()`**   | A unary modifier.                                                  | `-5`, `!false`        | `Consumed`, `Rhs`          |

### b. Chaining an Expression (The Infix Phase)

The parser already has a fully formed expression on its left (the `lhs`).<br>
It is looking for a token to bind that expression to the next one.<br>
> **Note:** The `lhs` isn't limited to expressions produced during the prefix phase.<br>
> It can be a synthetic AST node built and injected by other infix handlers.<br>
> This exact behavior is what enables parsing advanced formats like implied multiplication natively,<br>
> without lexer hacks (like inserting phantom nodes) or unnecessary lookaheads.<br>
> This guarantees `vpratt` always runs in strict $\mathcal{O}(N)$ time.

| Builder Method   | What it parses                       | Example                   | What your Handler receives              |
|:-----------------|:-------------------------------------|:--------------------------|:----------------------------------------|
| **`.infix()`**   | Standard binary operations.          | `a + b`,<br>`x * y`       | `lhs`, `Consumed`, `Rhs`                |
| **`.postfix()`** | Terminates the left-hand expression. | `x!`,<br>`i++`            | `lhs`, `Consumed`                       |
| **`.implied()`** | Adjacency binding with NO operator.  | `2x`                      | `lhs`, `Consumed`, `Resume`             |
| **`.juxt()`**    | Adjacency binding into a group.      | `2(a + b)`,<br>`fn(args)` | `lhs`, `Consumed`, `Enclosed`, `Resume` |

### 2. Handlers
An entry in the table (a format) maps to exactly one **Handler**—a user-defined function containing your custom parsing logic.

Crucially, **the format locks your function signature.** The compiler mathematically guarantees that your handler is handed exactly the tools required for that specific routing phase—no more, no less.

### 3. Capability Tokens
Because `vpratt` owns the main loop, you cannot just call `parser.next()` inside your handler. Instead, `vpratt` injects Capability Tokens directly into your handler's arguments.

These tokens offer restricted access to the core engine with absolute safety guarantees. Every token encapsulates a specific state transition:

| Capability Token | Purpose | Usage |
| :--- | :--- | :--- |
| **`Consumed`** | Grants access to the exact token that triggered the handler. | `consumed.token` |
| **`Rhs`** | Safely triggers the engine to parse the right side using the correct Right Binding Power (`rbp`). | `rhs.parse(self)?` |
| **`Subexpr`** | Enters a sub-expression parsing context with an `rbp` of `0`. | `reset.parse(self)?` |
| **`Enclosed`** | Enters a sub-expression parsing context, but also strictly consumes the expected closing delimiter. | `enclosed.parse(self)?` |
| **`Resume`** | Re-enters the infix phase with a synthetic `lhs` and the correct `rbp`. | `resume.parse(self, lhs)?` |