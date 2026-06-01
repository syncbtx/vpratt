//! This module allows the user to define formats and register handlers in a builder-style table.
//! This table serves as a compile time routing map which the core uses to create the pratt state machine.
//!
//! [`Table`]is the single source of truth for your grammar.
//!
//! Every operator, terminal, and keyword construct is declared here — along with its precedence,
//! associativity, and handler.
//!
//! Precedence numbers live in exactly one place.
//!
//! The `#[vpratt::parser]` macro reads this constant definition at compile time
//! and generates the optimized LBP loop and match dispatch for you.
//!
//! [`Table`] itself allocates nothing and carries no runtime state.
//!
//! # Formats
//!
//! Each builder method corresponds to a **format** — a grammatical role a token
//! can play.
//!
//! The format you declare determines the context type your handler
//! receives, which in turn determines exactly which parsing capabilities are
//! available.
//!
//! **Prefix phase** (parser holds no LHS):
//!
//! | Method | Context | Use for |
//! |---|---|---|
//! | [`.terminal()`](Table::terminal) | [`TerminalCtx`](crate::context::TerminalCtx) | Atomic values — numbers, identifiers, booleans |
//! | [`.group()`](Table::group) | [`GroupCtx`](crate::context::GroupCtx) | Delimited sub-expressions — `(.)`, `[.]` |
//! | [`.prefix()`](Table::prefix) | [`PrefixCtx`](crate::PrefixCtx) | Unary prefix operators — `-x`, `!flag` |
//! | [`.structural()`](Table::structural)   | [`StructuralCtx`](crate::context::StructuralCtx)  | Keyword constructs — `let`, `if`, `while` |
//!
//! **Infix phase** (parser holds a complete LHS):
//!
//! | Method | Context | Use for |
//! |---|---|---|
//! | [`.infix()`](Table::infix) | [`InfixCtx`](crate::context::InfixCtx) | Binary operators — `+`, `-`, `*`, `^` |
//! | [`.postfix()`](Table::postfix) | [`PostfixCtx`](crate::context::PostfixCtx) | Trailing operators — `x!`, `i++` |
//! | [`.implied()`](Table::implied) | [`ImpliedCtx`](crate::context::ImpliedCtx) | Token-adjacent juxtaposition — `2x`, `f a b` |
//! | [`.juxt()`](Table::juxt) | [`JuxtCtx`](crate::context::JuxtCtx) | Group-adjacent juxtaposition — `f(args)`, `2(a+b)` |
//!
//! # Precedence
//!
//! Higher numbers bind tighter. Conventional ranges:
//!
//! ```text
//! 10–30   low-precedence binary ops (sequences, assignment, comma)
//! 40–60   arithmetic and comparison
//! 70–80   unary prefix, function application
//! 90+     postfix, field access, array indexing
//! ```
//!
//! # Example
//!
//! ```rust
//! const TABLE: Table<Self> = Table::new()
//!     .terminal(Num(0.0),       Self::num)
//!     .group(LParen, RParen,    Self::grouped)
//!     .structural(Let,          Self::parselet)
//!     .prefix(70, Minus,        Self::negate)
//!     .infix(50, Left,  Plus,   Self::add)
//!     .infix(50, Left,  Minus,  Self::sub)
//!     .infix(60, Left,  Star,   Self::mul)
//!     .implied(80, Left, Ident(""), Self::application)
//!     .infix(90, Left,  Dot,    Self::arrayget);
//! ```

use core::marker::PhantomData;
use crate::core::{Associativity, Precedence, VprattCore};


#[derive(Debug, Clone, Copy)]
pub struct Table<P> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Table<P> {
    /// Initializes a new, empty routing table.
    ///
    /// Call builder methods on the returned value to register formats.
    ///
    /// Always assigned to a `const` field inside the `#[vpratt::parser]` impl block:
    /// ```rust
    /// const TABLE: Table<Self> = Table::new()
    ///     .terminal(...)
    ///     .infix;
    ///
    /// ```
    pub const fn new() -> Self {
        Self { _marker: PhantomData }
    }
    /// `.terminal()`
    ///
    /// Registers a terminal — a token that maps directly to a leaf AST node.
    ///
    /// Terminals have no binding power and do not recurse. The handler
    /// receives a [`TerminalCtx`](crate::context::TerminalCtx) carrying the consumed token.
    ///
    /// # Example
    ///
    /// ```rust
    /// .terminal(Num(0.0), Self::num)
    /// .terminal(True,     Self::literal)
    /// .terminal(Ident(""), Self::identifier)
    /// ```
    pub const fn terminal(
        self,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::TerminalCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.group()`
    ///
    /// Registers a grouped sub-expression bounded by matching delimiters.
    ///
    /// The engine drops precedence to 0 to parse the interior, then verifies
    /// and consumes the closing delimiter. The handler receives a [`GroupCtx`](crate::context::GroupCtx)
    /// carrying the consumed opening token and an [`Enclosed`](crate::capabilities::Enclosed) capability.
    ///
    /// Surfaces [`VprattError::UnmatchedDelimiter`](crate::error::VprattError::UnmatchedDelimiter) if the closing token is
    /// absent or does not match — never a silent wrong parse.
    ///
    /// # Example
    ///
    /// ```rust
    /// .group(LParen, RParen, Self::parenthesized)
    /// .group(LBracket, RBracket, Self::array_literal)
    /// ```
    pub const fn group(
        self,
        _open: P::PrattToken,
        _close: P::PrattToken,
        _handler: fn(&mut P, crate::context::GroupCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// Registers a unary prefix operator.
    ///
    /// The handler receives a [`PrefixCtx`](crate::context::PrefixCtx) carrying the consumed operator token
    /// and an [`Rhs`](crate::capabilities::Rhs) capability bound to `bp`. Calling `ctx.rhs.parse(self)?`
    /// parses the right-hand operand at the correct binding power.
    ///
    /// # Example
    ///
    /// ```rust
    /// .prefix(70, Minus,    Self::negate)
    /// .prefix(70, MinusDot, Self::fnegate)
    /// .prefix(70, Not,      Self::logical_not)
    /// ```
    pub const fn prefix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::PrefixCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.structural()`
    ///
    /// Registers a structural keyword construct.
    ///
    /// Structural handlers step outside the Pratt precedence loop entirely.
    /// The handler receives a [`StructuralCtx`](crate::context::StructuralCtx) carrying:
    ///
    /// - [`consumed`](crate::capabilities::Consumed) — the triggering keyword token
    /// - [`atom`](crate::capabilities::Atom) — parse a single atomic terminal without triggering infix rules
    /// - [`sub`](crate::capabilities::Subexpr) — parse a complete sub-expression from precedence 0
    ///
    /// All [`CtxUtils`](crate::CtxUtils) methods (
    /// [`expect`](crate::CtxUtils::expect),
    /// [`accept`](crate::CtxUtils::accept),
    /// [`series`](crate::CtxUtils::series),
    /// [`separated`](crate::CtxUtils::separated)
    /// ) are
    /// also available for safe, typed stream navigation.
    ///
    /// Structural handlers have no binding power — they always occupy the
    /// prefix phase and are never triggered from the infix loop.
    ///
    /// # Example
    ///
    /// ```rust
    /// .structural(Let,         Self::parselet)
    /// .structural(If,          Self::parseif)
    /// .structural(ArrayCreate, Self::parsearraycreate)
    /// ```
    pub const fn structural(
        self,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::StructuralCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.infix()`
    ///
    /// Registers a binary infix operator.
    ///
    /// The handler receives an [`InfixCtx`](crate::InfixCtx) carrying the already-parsed `lhs`,
    /// the consumed operator token, and an [`Rhs`](crate::capabilities::Rhs) capability. Associativity
    /// determines whether the RHS binding power is `bp` (left) or `bp - 1`
    /// (right).
    ///
    /// # Example
    ///
    /// ```rust
    /// .infix(50, Left,  Plus,  Self::add)
    /// .infix(50, Left,  Minus, Self::sub)
    /// .infix(60, Left,  Star,  Self::mul)
    /// .infix(60, Right, Caret, Self::power)  // right-associative
    /// ```
    pub const fn infix(
        self,
        _bp: Precedence,
        _assoc: Associativity,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::InfixCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.postfix()`
    ///
    /// Registers a unary postfix operator.
    ///
    /// The handler receives a [`PostfixCtx`](crate::PostfixCtx) carrying the already-parsed `lhs`
    /// and the consumed operator token. No further parsing capabilities are
    /// provided — the expression ends here.
    ///
    /// # Example
    ///
    /// ```rust
    /// .postfix(80, Bang,       Self::factorial)
    /// .postfix(80, PlusPlus,   Self::increment)
    /// ```
    pub const fn postfix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::PostfixCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.juxt()`
    ///
    /// Registers a group-adjacent juxtaposition operation.
    ///
    /// Triggered when the parser holds a complete LHS and encounters `open`
    /// as the next token — i.e., the LHS is immediately followed by a
    /// delimited group. The handler receives a [`JuxtCtx`](crate::JuxtCtx) carrying:
    ///
    /// - `lhs` — the already-parsed left operand
    /// - [`consumed`](crate::capabilities::Consumed) — the opening delimiter token
    /// - [`enclosed`](crate::capabilities::Enclosed)  — parse the interior and consume the closing delimiter
    /// - [`resume`](crate::capabilities::Resume)  — re-enter the infix loop with a synthetic LHS
    ///
    /// # Example
    ///
    /// ```rust
    /// .juxt(80, LParen, RParen, Self::call) // f(args)
    /// ```
    pub const fn juxt(
        self,
        _bp: Precedence,
        _open: P::PrattToken,
        _close: P::PrattToken,
        _handler: fn(&mut P, crate::context::JuxtCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }

    /// `.implied()`
    ///
    /// Registers a token-adjacent juxtaposition operation.
    ///
    /// Triggered when the parser holds a complete LHS and encounters `token`
    /// as the next token — i.e., the LHS is immediately followed by an
    /// adjacent token with no operator between them. The handler receives
    /// an [`ImpliedCtx`](crate::context::ImpliedCtx)  carrying:
    ///
    /// - `lhs` — the already-parsed left operand
    /// - [`consumed`](crate::capabilities::Consumed)  — the triggering adjacent token
    /// - [`seed`](crate::capabilities::Seed)  — re-enter the NUD phase with the consumed token
    /// - [`resume`](crate::capabilities::Resume)  — re-enter the infix loop with a synthetic LHS
    ///
    /// Together, [`seed`](crate::capabilities::Seed)  and [`resume`](crate::capabilities::Resume) enable correct O(N) handling of
    /// function application and implied multiplication without lookahead
    /// or extra passes.
    ///
    /// # Example
    ///
    /// ```rust
    /// // ML-style application: f a b c
    /// .implied(80, Left, Ident(""),   Self::application)
    /// .implied(80, Left, Integer(0),  Self::application)
    /// .implied(80, Left, Float(0.0),  Self::application)
    ///
    /// // Implied multiplication: 2x
    /// .implied(70, Left, Ident(""),   Self::implied_mul)
    /// ```
    pub const fn implied(
        self,
        _bp: Precedence,
        _assoc: Associativity,
        _token: P::PrattToken,
        _handler: fn(&mut P, crate::context::ImpliedCtx<P>) -> crate::core::Result<P>,
    ) -> Self { self }
}