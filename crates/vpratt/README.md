# <mark style="background-color:#99621E">`vpratt`</mark>

<hr style="height:1px; border-width:0; background-color:grey; background-image:linear-gradient(to right, rgba(0,0,0,0), rgba(0,0,0,0.75), rgba(0,0,0,0));">

<small>

*If you can read a precedence table, you can write a parser.*

`vpratt` is a product of pushing [Vaughan Pratt](https://en.wikipedia.org/wiki/Vaughan_Pratt)'s 1973 [algorithm](https://en.wikipedia.org/wiki/Operator-precedence_parser) to its very limits to fully harness its potential for modern applications.  
Where most implementations stop at simple arithmetic, `vpratt` keeps going — structural constructs, juxtaposition, enclosed expressions, complex grammars — all handled natively, without leaving the safety of the framework.

Not all grammars are purely precedence-based, so how do you parse bindings, conditionals or a list of arguments?  
That is where [recursive descent](https://en.wikipedia.org/wiki/Recursive_descent_parser) parsers excel and Pratt parsers stall.  
`vpratt` bridges the two, allowing you to design and implement virtually any expression-based language and DSL natively, courtesy of the ancient algorithm.

`vpratt` can comfortably do everything the pure Pratt algorithm is capable of and more — with compile-time safety, correctness guarantees, minimal boilerplate, and an uncompromising developer experience.

`vpratt` is extremely lightweight and follows a strict zero-allocation rule. The framework completely gets out of the way so you can approach your domain however you see fit. That means you choose your memory management model, AST, and error types — reporting and diagnostics all integrated intuitively.

---

###### *who is vpratt for?*

`vpratt` is for:
- Compiler and interpreter authors
- DSL and query language designers
- [Nerds](../../Reference.md)
- Anyone who has hand-rolled a Pratt parser and felt the absence of a safety net

`vpratt` is not:
- a lexer
- a code generator
- a regex replacement
- a parser combinator
- opinionated — at least not about your domain

---

### Examples

- [**calc**](../../examples/calc/src/main.rs) — An arithmetic parser covering the full range of regular operators: addition, subtraction, multiplication, division, negation, power, factorial. The foundations, done right.

- [**math-lang**](../../examples/math-lang/src/main.rs) — A mathematical expression language pushing juxtaposition and implied operators to their limits. Syntax just like how you write math on paper — `sin x`, `2x`, `(a + b)(a + b)`.

- [**query**](../../examples/query/src/main.rs) — A search query DSL with boolean logic, field syntax, and grouping. A practical example of `vpratt` beyond compiler territory.

- [**mincaml compiler**](../../examples/mincaml/src/parser.rs) — A complete frontend for MinCaml, a real ML-family language. Let bindings, tuples, arrays, and function application in under 300 lines.

---

### Installation

```sh
cargo add vpratt
```

---

[Reference](../../Reference.md)

<hr style="height:1px; border-width:0; background-color:grey; background-image:linear-gradient(to right, rgba(0,0,0,0), rgba(0,0,0,0.75), rgba(0,0,0,0));">

###### Artwork. Yay!

</small>

![vpratt banner](../../vpratt%20banner%20dark.svg)