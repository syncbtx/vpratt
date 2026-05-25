//! This module defines the various context types passed to  handlers.
//! Each context corresponds to a specific registration method on the parsing [`Table`](crate::dsl::Table)
//! (e.g., `TerminalCtx` for `.terminal()`, `InfixCtx` for `.infix()`).
//!
//! These structures encapsulate the state of the parse at the moment a handler is invoked.
//! Depending on the syntactic construct, they provide the consumed token, any previously
//! parsed left-hand side (`lhs`), and specific parsing capabilities (such as [`Rhs`],
//! [`Enclosed`], or [`Subexpr`]) required to complete the rule.
pub mod utils;

use core::marker::PhantomData;
use crate::{Atom, Consumed, Enclosed, Resume, Rhs, Seed, Subexpr};
use crate::core::VprattCore;

/// Context provided to handlers registered via [`.terminal()`](crate::dsl::Table::terminal).
///
/// Represents a standalone atomic value.
///
/// The handler receives the consumed token and is responsible for constructing the corresponding AST node.
///
/// No further parsing capabilities are provided — terminals do not recurse.
///
/// **Fields:** [`consumed`](Consumed)
///
/// **When to use:** Integers, floats, booleans, identifiers — any token
/// that maps directly to a leaf node in the AST.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn literal(&mut self, ctx: TerminalCtx<Self>) -> vpratt::Result<Self> {
///     match ctx.consumed.token.kind {
///         TokenKind::Integer(n) => Ok(Expr::Int(n)),
///         TokenKind::Float(f)   => Ok(Expr::Float(f)),
///         _ => unreachable!(),
///     }
/// }
/// ```
pub struct TerminalCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.group()`](crate::dsl::Table::group).
///
/// Represents a delimited sub-expression.
///
/// The `enclosed` capability parses the interior from precedence 0 and consumes the closing delimiter.
///
/// Surfaces a typed error if the closing delimiter is missing.
///
/// **Fields:** `consumed`, `enclosed`
///
/// **When to use:** Parenthesised expressions `(...)`, array literals `[...]`,
/// or any balanced delimiter pair.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn parenthesized(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self> {
///     let (inner, _) = ctx.enclosed.parse(self)?;
///     Ok(inner)
/// }
pub struct GroupCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub enclosed: Enclosed<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.structural()`](crate::dsl::Table::structural).
///
/// Represents a keyword-driven construct that falls outside standard precedence-based parsing.
///
/// Provides two parsing capabilities:
/// - [`atom`](Atom) — parse a single atomic terminal (identifier, literal) without triggering infix rules.
///
/// - [`sub`](Subexpr) — parse a complete sub-expression from precedence 0, as if starting the parser fresh.
///
/// Additionally, all [`CtxUtils`](crate::CtxUtils) methods are available on this context for safe stream navigation.
///
/// **Fields:** [`consumed`](Consumed), [`atom`](Atom), [`sub`](Subexpr)
///
/// **When to use:**
///
/// Language keywords with irregular structure — `let`,
/// `if`, `while`, `match`, `Array.create`.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn parseif(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
///     let cond = self.arena.alloc(ctx.sub.parse(self)?);
///     ctx.expect(self, Then)?;
///     let then = self.arena.alloc(ctx.sub.parse(self)?);
///     ctx.expect(self, Else)?;
///     let else_ = self.arena.alloc(ctx.sub.parse(self)?);
///     Ok(Expression { kind: ExprKind::If { cond, then, else_ }, .. })
/// }
/// ```
pub struct StructuralCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub atom:    Atom<P>,
    pub sub:     Subexpr<P>,
    pub _marker: PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.prefix()`](crate::dsl::Table::prefix).
///
/// Represents a unary prefix operator.
///
/// The `rhs` capability parses the right-hand operand at the operator's registered binding power.
///
/// **Fields:** [`consumed`](Consumed), [`rhs`](Rhs)
///
/// **When to use:** Unary operators like `-x`, `!flag`, `~bits`.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn negate(&mut self, ctx: PrefixCtx<Self>) -> vpratt::Result<Self> {
///     Ok(Expr::Neg(Box::new(ctx.rhs.parse(self)?)))
/// }
/// ```
pub struct PrefixCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub rhs:      Rhs<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.infix()`](crate::dsl::Table::infix).
///
/// Represents a binary infix operator.
///
/// The `lhs` is the already-parsed left-hand operand.
///
/// The `rhs` capability parses the right-hand operand
/// at the operator's registered binding power, respecting associativity.
///
/// **Fields:** `lhs`, [`consumed`](Consumed), [`rhs`](Rhs)
///
/// **When to use:** Binary operators like `+`, `-`, `*`, `/`, `^`,
/// comparison operators, assignment, sequencing.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
///     Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
/// }
/// ```
pub struct InfixCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub rhs:      Rhs<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.postfix()`](crate::dsl::Table::postfix).
///
/// Represents a trailing unary operator.
///
/// The `lhs` is the already-parsed operand.
///
/// No further parsing capabilities are provided — the expression ends here.
///
/// **Fields:** `lhs`, [`consumed`](Consumed)
///
/// **When to use:** Trailing operators like `x!`, `i++`, `val?`.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn factorial(&mut self, ctx: PostfixCtx<Self>) -> vpratt::Result<Self> {
///     Ok(Expr::Factorial(Box::new(ctx.lhs)))
/// }
/// ```
pub struct PostfixCtx<'a, P: VprattCore> {
    pub  lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.juxt()`](crate::dsl::Table::juxt).
///
/// Represents juxtaposition involving an enclosed group — for
/// example `f(args)` where `f` is the LHS and `(args)` is the adjacent group.
///
/// The [`enclosed`](Enclosed) capability parses the interior of the group.
///
/// The [`resume`](Resume) capability re-enters the infix loop with the result as a synthetic LHS,
/// allowing further chaining.
///
/// **Fields:** `lhs`, [`consumed`](Consumed), [`enclosed`](Enclosed), [`resume`](Resume)
///
/// **When to use:** Function call syntax via adjacency — `f(x + 1)`,
/// `array(index)`, or any implied operation involving a delimited group.
pub struct JuxtCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub enclosed: Enclosed<P>,
    pub resume:   Resume<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via [`.implied()`](crate::dsl::Table::implied).
///
/// Represents pure token juxtaposition — adjacency without
/// a delimiter. The [`seed`](Seed) capability re-enters the NUD phase with the
/// already-consumed token to produce the first argument.
///
/// The [`resume`](Resume) capability re-enters the infix loop with that argument as a synthetic
/// LHS, allowing further arguments to chain.
///
/// **Fields:** `lhs`, [`consumed`](Consumed), [`seed`](Seed), [`resume`](Resume)
///
///
/// **When to use:** ML-style curried function application `f a b c`,
/// implied multiplication `2x`, or any operator-free juxtaposition.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
///     let token = ctx.consumed.token;
///     let arg   = ctx.seed.parse(self, token)?;
///     let full  = ctx.resume.parse(self, arg)?;
///     Ok(Expr::Apply(Box::new(ctx.lhs), Box::new(full)))
/// }
/// ```
pub struct ImpliedCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub seed:     Seed<P>,
    pub resume:   Resume<P>,
    pub _marker:  PhantomData<&'a ()>,
}