//! This module contains the mathematical foundation and algorithms of the framework:
//!
//! - [`Precedence`] — the numerical weight type for operator binding power
//! - [`Associativity`] — left or right binding for same-precedence operators
//! - [`VprattCore`] — the internal trait the `#[vpratt::parser]` macro implements
//! - [`Result`] — the convenience result type alias used by all handlers
//!
//! As an end-user you rarely interact with this module directly.
//!
//! It is surfaced here for documentation completeness and for advanced use cases
//! such as implementing custom parser wrappers.

use crate::error::VprattError;

/// The numerical weight used to determine the Left Binding Power (LBP) of operators.
///
/// Higher numbers bind tighter. Conventional ranges used in vpratt grammars:
///
/// ```text
/// 10–30   low-precedence binary ops — sequences, assignment, comma
/// 40–60   arithmetic and comparison ops
/// 70–80   unary prefix, function application
/// 90+     postfix, field access, array indexing
/// ```
///
/// The default Precedence uses u16.
/// With the bp_u8 feature `features = ["bp_u8"]` you can opt to use u8 instead
/// It is recommended you use factors of 10 for you Precedence to avoid collisions: '10, 20, 100, 110`
///
/// Precedence numbers live in exactly one place: the [`Table`](crate::dsl::Table)
/// declaration.
///
/// When there is a precedence bug, the table is the only place to look.

#[cfg(feature = "bp_u8")]
pub type Precedence = u8;
#[cfg(not(feature = "bp_u8"))]
pub type Precedence = u16;

/// Convenience result type alias for handler return types.
///
/// Resolves to `Result<P::Output, P::Error>` for a given parser `P`.
///
/// When a custom error type is provided via `error = MyError` in
/// `#[vpratt::parser]`.
///
/// `vpratt::Result<Self>` resolves to
/// `Result<Output, MyError>` automatically.
///
/// # Example
///
/// ```rust
/// #[vpratt::handler]
/// fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
///     Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
/// }
/// ```

pub type Result<P> = core::result::Result<
    <P as VprattCore>::Output,
    <P as VprattCore>::Error,
>;

/// The internal trait powering the Left Binding Power (LBP) loop.
///
/// This trait is automatically implemented for your parser struct by the
/// `#[vpratt::parser]` attribute macro. You never implement it manually.
/// It contains the hidden dispatch mechanisms and LBP state machine that
/// drive Pratt parsing without allocating.
///
/// # Associated Types
///
/// The four associated types form the complete type signature of your parser:
///
/// - [`VprattCore::Item`] — the raw type yielded by your stream iterator
/// - [`VprattCore::PrattToken`] — the routing type extracted from [`Item`](VprattCore::Item)  to navigate the table
/// - [`VprattCore::Output`]  — the AST node type your handlers return
/// - [`VprattCore::Error`]  — your error type, defaulting to `VprattError<Item, PrattToken>`
///
/// All four are inferred from the `#[vpratt::parser]` arguments — you never
/// write them yourself.
pub trait VprattCore {
    /// The raw token type yielded by the stream iterator.
    ///
    /// When your stream item and routing token are the same type (e.g. a plain
    /// `TokenKind` enum), [`Item`](VprattCore::Item) and [`PrattToken`](VprattCore::PrattToken)  are identical.
    ///
    /// When your stream yields rich tokens with spans and metadata (e.g. `Token<'a>`),
    /// [`Item`](VprattCore::Item)  is the rich type and `PrattToken` is the extracted routing enum.
    type Item;

    /// The routing token type used to navigate the precedence table.
    ///
    /// Must implement `Clone + Copy + PartialEq`. Usually a plain `TokenKind`
    /// enum.
    ///
    /// The `extract` argument on `#[vpratt::parser]` (or the
    /// `#[vpratt::token]` macro) defines how this is derived from `Item`.
    type PrattToken: Clone + Copy + PartialEq;

    /// The AST node type returned by all handlers.
    ///
    /// Set via `output = YourType` in `#[vpratt::parser]`. Can carry
    /// lifetimes and be arena-allocated — vpratt imposes no constraints
    /// on its shape.
    type Output;

    /// The error type returned by all handlers.
    ///
    /// Defaults to `VprattError<Item, PrattToken>`. Set via `error = YourType`
    /// in `#[vpratt::parser]`. Must implement
    /// `From<VprattError<Item, PrattToken>>` when a custom type is provided.
    type Error;

    #[doc(hidden)]
    fn __next__(&mut self) -> Option<Self::Item>;
    #[doc(hidden)]
    fn __peek__(&mut self) -> Option<&Self::Item>;
    #[doc(hidden)]
    fn __extract__(token: &Self::Item) -> Self::PrattToken;
    #[doc(hidden)]
    fn __lbp__(token: &Self::Item) -> Precedence;
    #[doc(hidden)]
    fn __nud__(&mut self, token: Self::Item) -> Result<Self>;
    #[doc(hidden)]
    fn __led__(&mut self, lhs: Self::Output, token: Self::Item) -> Result<Self>;
    #[doc(hidden)]
    fn __convert_error__(err: VprattError<Self::Item, Self::PrattToken>) -> Self::Error;

    /// The core Pratt parsing algorithm.
    ///
    /// Consumes one token from the stream, routes it through the NUD
    /// (prefix/terminal) dispatch, then enters the infix loop.
    ///
    /// Called by the generated entry point method and by capability tokens like
    /// ([`Rhs`](crate::Rhs), [`Subexpr`](crate::Subexpr)) when they need to recurse.
    #[doc(hidden)]
    fn __pratt_parse_internal__(&mut self, minbp: Precedence) -> Result<Self> {
        let lhs = {
            let token = match self.__next__() {
                Some(token) => token,
                None => return Err(Self::__convert_error__(VprattError::UnexpectedEOF)),
            };
            self.__nud__(token)?
        };
        self.__pratt_resume_infix__(lhs, minbp)
    }

    /// The Left Binding Power (LBP) infix loop.
    ///
    /// Continuously consumes tokens from the stream as long as their binding
    /// power exceeds `rbp`.
    ///
    /// Called after the NUD phase and by the [`Resume`](crate::Resume)
    /// capability token when re-entering the infix phase with a synthetic LHS.
    #[doc(hidden)]
    fn __pratt_resume_infix__(&mut self, mut lhs: Self::Output, rbp: Precedence) -> Result<Self> {
        loop {
            let lbp = match self.__peek__() {
                Some(peeked) => Self::__lbp__(peeked),
                None => break,
            };

            if lbp <= rbp { break; }

            let token = self.__next__().unwrap();
            lhs = self.__led__(lhs, token)?;
        }
        Ok(lhs)
    }
}

/// Defines how operators of the same precedence level bind to each other.
///
/// Passed as the second argument to `.infix()` and `.implied()` in the
/// [`Table`](crate::dsl::Table) declaration.
///
/// # Example
///
/// ```rust
/// use vpratt::Associativity::{Left, Right};
///
/// Table::new()
///     .infix(50, Left,  Plus,  Self::add)   // (a + b) + c
///     .infix(60, Right, Caret, Self::power) // a ^ (b ^ c)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    /// Left-associative: `a - b - c` parses as `(a - b) - c`.
    Left,
    /// Right-associative: `a ^ b ^ c` parses as `a ^ (b ^ c)`.
    Right,
}