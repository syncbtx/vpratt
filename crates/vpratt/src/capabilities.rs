//! This module defines the granular capability tokens (such as [`Rhs`], [`Enclosed`], and [`Seed`])
//! that are embedded within handler context structures.
//!
//! Rather than exposing raw, unrestricted parsing loops or mutable state directly to the user,
//! `vpratt` utilizes these tokens to enforce structural correctness at the type level.
//! Each token grants a highly specific parsing action—whether that is driving the right-hand
//! side of an infix expression respecting binding power, consuming an atomic terminal, or
//! safely re-entering the Pratt loop after a juxtaposed token.
//!
//! By restricting operations to only what is valid for a given syntactic construct, these
//! capabilities prevent common recursive-descent errors like incorrect precedence propagation
//! or silent failures on unmatched delimiters.

use core::marker::PhantomData;
use crate::{core::{Precedence,VprattCore}};
use crate::error::VprattError;


/// A wrapper proving that a specific token was consumed by the engine
/// and is ready for use in a handler.
///
/// Accessible on all context types as `ctx.consumed`.
///
/// The raw token is available as `ctx.consumed.token`.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn my_op(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
///     let span = ctx.consumed.token.span; // access the triggering token
///     // ...
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Consumed<T> {
    pub token: T
}

impl<T> Consumed<T> {
    #[doc(hidden)]
    pub fn __new__(token: T) -> Self { Self { token } }
}


/// A capability token that parses a single atomic terminal from the stream.
///
/// Accessible as `ctx.atom` on [`StructuralCtx`](crate::StructuralCtx).
///
/// Drives the engine's NUD
/// (prefix) router for exactly one token — equivalent to parsing a terminal
/// value without triggering any infix rules.
///
/// Use this when a structural construct needs to consume a bare identifier,
/// number, or other atomic value without entering a full expression parse.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn parselet(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
///     let name = ctx.atom.parse(self)?; // consumes exactly one terminal
///     // ...
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Atom<P: VprattCore> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Atom<P> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn __new__() -> Self { Self { _marker: PhantomData } }

    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> crate::core::Result<P> {
        let token = p.__next__().ok_or_else(|| P::__convert_error__(VprattError::UnexpectedEOF))?;
        p.__nud__(token)
    }
}


/// A capability token that re-enters the NUD phase with a pre-consumed token.
///
/// Accessible as `ctx.seed` on [`ImpliedCtx`](crate::ImpliedCtx). Because the triggering token
/// has already been consumed by the engine before the handler is called,
/// [`Seed`] feeds it back into the prefix router to produce a valid AST node.
///
/// This is the primitive that enables native function application and implied
/// multiplication in a single O(N) pass.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
///     let token = ctx.consumed.token;
///     let arg   = ctx.seed.parse(self, token)?; // re-enter NUD with consumed token
///     let full  = ctx.resume.parse(self, arg)?;
///     Ok(Expr::Apply(Box::new(ctx.lhs), Box::new(full)))
/// }
/// ```
pub struct Seed<P: VprattCore> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Seed<P> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn __new__() -> Self { Self { _marker: PhantomData } }

    #[inline(always)]
    pub fn parse(&self, p: &mut P, token: P::Item) -> crate::core::Result<P> {
        p.__nud__(token)
    }
}


/// A capability token that parses the right-hand side of an expression at
/// the correct binding power.
///
/// Accessible as `ctx.rhs` on [`PrefixCtx`](crate::PrefixCtx) and [`InfixCtx`](crate::InfixCtx).
///
/// The binding power is set by the engine from the format's table declaration — you never write it manually.
///
/// Calling `.parse(self)?` on a capability token  drives the Pratt loop until a token
/// with lower or equal binding power is encountered.
///
/// It should be noted that not all capabilities tokens have the .`parse(self)?` function.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
///     Ok(Expr::Add(
///         Box::new(ctx.lhs),
///         Box::new(ctx.rhs.parse(self)?), // parses RHS at Plus's binding power
///     ))
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Rhs<P: VprattCore> {
    pub rbp: Precedence,
    pub _marker: PhantomData<P>
}

impl<P: VprattCore> Rhs<P> {
    #[doc(hidden)]
    pub fn __new__(rbp: Precedence) -> Self { Rhs { rbp, _marker: PhantomData } }

    /// Executes the Pratt loop to resolve the right-hand side of the expression.
    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> crate::core::Result<P> {
        p.__pratt_parse_internal__(self.rbp)
    }
}

/// A capability token that parses a complete sub-expression from precedence 0.
///
/// Accessible as `ctx.sub` on [`StructuralCtx`](crate::StructuralCtx).
///
/// Resets the engine's binding power context temporarily, allowing a structural handler to parse a fresh,
/// unbounded expression — as if starting the parser from scratch.
///
/// Use this for parsing the branches of `if`, the value and body of `let`,
/// or any sub-expression that should not be constrained by the surrounding
/// precedence context.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn parseif(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
///     let cond = ctx.sub.parse(self)?; // fresh expression, precedence 0
///     ctx.expect(self, Then)?;
///     let then = ctx.sub.parse(self)?;
///     // ...
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Subexpr<P: VprattCore> {
    pub _marker: PhantomData<P>
}

impl<P: VprattCore> Subexpr<P> {
    #[doc(hidden)]
    pub fn __new__() -> Self { Subexpr { _marker: PhantomData } }

    /// Executes the Pratt loop from a baseline precedence of 0.
    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> crate::core::Result<P> {
        p.__pratt_parse_internal__(0)
    }
}

/// A capability token that re-enters the infix phase with a synthetic LHS.
///
/// Accessible as `ctx.resume` on [`ImpliedCtx`](crate::ImpliedCtx) and [`JuxtCtx`](crate::JuxtCtx).
///
/// After [`seed`](Seed) or [`enclosed`](Enclosed) has produced an initial AST node, [`resume`](Resume) feeds it back
/// into the infix loop to chain further operations — allowing trailing
/// operators like `^2` in `f x^2` to bind correctly before the implied
/// operation collapses.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn application(&mut self, ctx: ImpliedCtx<Self>) -> vpratt::Result<Self> {
///     let token = ctx.consumed.token;
///     let arg   = ctx.seed.parse(self, token)?;
///     let full  = ctx.resume.parse(self, arg)?; // re-enter infix with arg as LHS
///     Ok(Expr::Apply(Box::new(ctx.lhs), Box::new(full)))
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Resume<P: VprattCore> {
    pub rbp: Precedence,
    pub _marker: PhantomData<P>
}

impl<P: VprattCore> Resume<P> {
    #[doc(hidden)]
    pub fn __new__(rbp: Precedence) -> Self { Resume { rbp, _marker: PhantomData } }

    /// Resumes the infix loop, using the provided `lhs` as the base expression.
    #[inline(always)]
    pub fn parse(&self, p: &mut P, lhs: P::Output) -> crate::core::Result<P> {
        p.__pratt_resume_infix__(lhs, self.rbp)
    }
}

/// A capability token that parses a sub-expression and consumes its closing delimiter.
///
/// Accessible as `ctx.enclosed` on [`GroupCtx`](crate::GroupCtx) and [`JuxtCtx`](crate::JuxtCtx).
///
/// Parses the interior of a delimited group from precedence 0, then verifies and
/// consumes the expected closing token.
///
/// Returns a tuple of the parsed AST node and the raw closing token.
///
/// Surfaces `VprattError::UnmatchedDelimiter` if the closing token is absent
/// or does not match — never a silent wrong parse.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn grouped(&mut self, ctx: GroupCtx<Self>) -> vpratt::Result<Self> {
///     let (inner, _close) = ctx.enclosed.parse(self)?;
///     Ok(inner)
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Enclosed<P: VprattCore> {
    pub expected_close: P::PrattToken,
}

impl<P: VprattCore> Enclosed<P> {
    #[doc(hidden)]
    pub fn __new__(expected_close: P::PrattToken) -> Self {
        Self { expected_close }
    }

    /// Parses the interior expression and consumes the closing delimiter.
    ///
    /// Returns a tuple containing the generated AST node and the raw closing token.
    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> core::result::Result<(P::Output, P::Item), P::Error> {
        let inner = p.__pratt_parse_internal__(0)?;
        let token = match p.__next__(){
            Some(token) => token,
            None => return Err(P::__convert_error__(VprattError::UnexpectedEOF)),
        };

        if P::__extract__(&token) == self.expected_close {
            Ok((inner, token))
        }
        else {
            Err(P::__convert_error__(VprattError::UnmatchedDelimiter(self.expected_close, token)))
        }
    }
}