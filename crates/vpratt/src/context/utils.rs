//! Utility methods available on all vpratt context types.
//!
//! These extend vpratt's correctness guarantees into structural parsing —
//! the parts of a grammar that aren't driven by precedence but still need
//! safe, typed stream navigation. 
//! 
//! The same guarantee philosophy as the core applies: typed errors, no silent wrong parses.
//!
//! All methods take `p: &mut P` as the first argument, consistent with
//! how capability tokens work — the context is the access point, the
//! parser drives the stream.
//!
//! ## Available On
//!
//! - [`TerminalCtx`](crate::context::TerminalCtx)
//! - [`GroupCtx`](crate::context::GroupCtx)
//! - [`PrefixCtx`](crate::context::PrefixCtx)
//! - [`InfixCtx`](crate::context::InfixCtx)
//! - [`PostfixCtx`](crate::context::PostfixCtx)
//! - [`StructuralCtx`](crate::context::StructuralCtx)
//! - [`ImpliedCtx`](crate::context::ImpliedCtx)
//! - [`JuxtCtx`](crate::context::JuxtCtx)
//!
//! ## Methods
//!
//! - [`CtxUtils::expect`] — consume the next token or fail with a typed error
//! - [`CtxUtils::accept`] — optionally consume the next token
//! - [`CtxUtils::series`] — collect a token class until a delimiter
//! - [`CtxUtils::separated`] — collect a token class with a separator and end token
//!
//! ## Example
//!
//! ```rust
//! #[vpratt::handler]
//! fn parselet(&mut self, ctx: StructuralCtx<Self>) -> vpratt::Result<Self> {
//!     let is_rec = ctx.accept(self, Recursive)?.is_some();
//!     let name   = ctx.expect(self, Ident(""))?;
//!     let args   = ctx.series(self, Ident(""), Equals)?;
//!     ctx.expect(self, Equals)?;
//!     let value  = ctx.sub.parse(self)?;
//!     ctx.expect(self, In)?;
//!     let body   = ctx.sub.parse(self)?;
//!     // ...
//! }
//! ```

use alloc::vec::Vec;
use crate::Consumed;
use crate::core::VprattCore;
use crate::error::VprattError;

/// Utility methods available on all vpratt context types.
///
/// Extend vpratt's correctness guarantees into structural parsing. Import
/// this trait to use [`expect`](CtxUtils::expect), [`accept`](CtxUtils::accept), [`series`](CtxUtils::series), and [`separated`](CtxUtils::separated) on any
/// context object inside a `#[vpratt::handler]`.
pub trait CtxUtils<P: VprattCore> {
    /// Consume the next token if it matches `expected`, otherwise return a typed error.
    ///
    /// Surfaces [`VprattError::ExpectedTokenMismatch`] if the token does not
    /// match, and [`VprattError::UnexpectedEOF`] if the stream is exhausted.
    ///
    /// # Example
    ///
    /// ```rust
    /// ctx.expect(self, Then)?;
    /// ctx.expect(self, In)?;
    /// ```
    fn expect(
        &self,
        p: &mut P,
        expected: P::PrattToken,
    ) -> core::result::Result<Consumed<P::Item>, P::Error> {
        match p.__peek__() {
            None => Err(P::__convert_error__(VprattError::UnexpectedEOF)),
            Some(item) => {
                let extracted = P::__extract__(item);
                if extracted == expected {
                    let token = p.__next__().unwrap();
                    Ok(Consumed::__new__(token))
                } else {
                    let actual = p.__next__().unwrap();
                    Err(P::__convert_error__(VprattError::ExpectedTokenMismatch(
                        expected,
                        actual,
                    )))
                }
            }
        }
    }

    /// Optionally consume the next token if it matches `expected`.
    ///
    /// Returns `Some(Consumed)` if the token matches, `None` if it does not.
    /// 
    /// The stream is left untouched on a non-match. Never fails — the `Result`
    /// wrapper exists solely to allow `?` propagation of internal stream errors.
    ///
    /// # Example
    ///
    /// ```rust
    /// let is_rec = ctx.accept(self, Recursive)?.is_some();
    /// ```
    fn accept(
        &self,
        p: &mut P,
        expected: P::PrattToken,
    ) -> core::result::Result<Option<Consumed<P::Item>>, P::Error> {
        match p.__peek__() {
            Some(item) if P::__extract__(item) == expected => {
                let token = p.__next__().unwrap();
                Ok(Some(Consumed::__new__(token)))
            }
            _ => Ok(None),
        }
    }

    /// Collect a sequence of tokens matching `of` until `delimiter` is seen.
    ///
    /// The delimiter is **not** consumed — it is left in the stream for the
    /// caller to handle. 
    /// 
    /// Returns an empty `Vec` if the stream immediately contains the delimiter or a non-matching token.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Parsing the argument list in: let rec f x y z =
    /// //
    /// // stream: [Ident("x"), Ident("y"), Ident("z"), Equals, ...]
    /// let args = ctx.series(self, Ident(""), Equals)?;
    /// // args:   [Consumed(x), Consumed(y), Consumed(z)]
    /// // stream: [Equals, ...]
    /// ```
    fn series(
        &self,
        p: &mut P,
        of: P::PrattToken,
        delimiter: P::PrattToken,
    ) -> core::result::Result<Vec<Consumed<P::Item>>, P::Error> {
        let mut items = Vec::new();
        loop {
            match p.__peek__() {
                Some(item) if P::__extract__(item) == delimiter => break,
                None => break,
                Some(item) if P::__extract__(item) == of => {
                    let token = p.__next__().unwrap();
                    items.push(Consumed::__new__(token));
                }
                _ => break,
            }
        }
        Ok(items)
    }

    /// Collect tokens matching `of`, separated by `sep`, until `end` is consumed.
    ///
    /// The `end` delimiter **is** consumed. 
    /// 
    /// Leading and trailing separators are
    /// not accepted — the structure must be `of (sep of)* end`. 
    /// 
    /// Returns [`VprattError::UnexpectedEOF`](VprattError) if the stream is exhausted before `end`.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Parsing the ident list in: let (x, y, z) =
    /// // Assuming `(` has already been consumed.
    /// //
    /// // stream: [Ident("x"), Comma, Ident("y"), Comma, Ident("z"), RParen, ...]
    /// let idents = ctx.separated(self, Ident(""), Comma, RParen)?;
    /// // idents: [Consumed(x), Consumed(y), Consumed(z)]
    /// // stream: [...]  — RParen consumed
    /// ```
    fn separated(
        &self,
        p: &mut P,
        of: P::PrattToken,
        sep: P::PrattToken,
        end: P::PrattToken,
    ) -> core::result::Result<Vec<Consumed<P::Item>>, P::Error> {
        let mut items = Vec::new();

        loop {
            match p.__peek__() {
                Some(item) if P::__extract__(item) == end => {
                    p.__next__();
                    break;
                }
                None => {
                    return Err(P::__convert_error__(VprattError::UnexpectedEOF));
                }
                Some(item) if P::__extract__(item) == of => {
                    let token = p.__next__().unwrap();
                    items.push(Consumed::__new__(token));
                }
                _ => {
                    let actual = p.__next__().unwrap();
                    return Err(P::__convert_error__(VprattError::UnexpectedToken(actual)));
                }
            }

            match p.__peek__() {
                Some(item) if P::__extract__(item) == sep => {
                    p.__next__();
                }
                Some(item) if P::__extract__(item) == end => {
                    p.__next__();
                    break;
                }
                None => {
                    return Err(P::__convert_error__(VprattError::UnexpectedEOF));
                }
                _ => {
                    let actual = p.__next__().unwrap();
                    return Err(P::__convert_error__(VprattError::UnexpectedToken(actual)));
                }
            }
        }

        Ok(items)
    }
}


impl<'a, P: VprattCore> CtxUtils<P> for crate::context::TerminalCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::GroupCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::StructuralCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::PrefixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::InfixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::PostfixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::ImpliedCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::context::JuxtCtx<'a, P> {}