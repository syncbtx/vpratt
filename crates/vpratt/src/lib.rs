//! # vpratt Core
//!
//! This module contains the core of the `vpratt` framework.
//! It provides the raw, zero-allocation Left Binding Power (LBP) loops,
//! state-management capability tokens, and the static routing table definition.
//!
//! ## Architecture
//! `vpratt` is split into two crates:
//! 1. **The Core (This Crate):** Provides the mathematical primitives and the `VprattCore` trait.
//! 2. **The Macros (`vpratt-macros`):** Reads the user's `Table` definition and automatically implements
//!    `VprattCore`, wiring the user's standard Rust functions into the engine's LBP loop.
//!
//! **Note:** As an end-user, you should rarely need to interact with `VprattCore` directly.
//! You interact with this crate primarily through the Capability Tokens (`Rhs`, `Resume`, `Enclosed`)
//! passed to your handler functions.
//! 
//! By commiting to an entry type provided by the Table's builder methods, you lock the handler signature
//! such that you have access to a subset of the handlers which reduces the possibility of errors
//! handlers have a 1:1 mapping with builder methods
//!

#![no_std]
extern crate alloc;

pub mod prelude;

use alloc::vec::Vec;
use core::marker::PhantomData;
use core::fmt::{Debug, Display, Formatter, Result as FmtResult};
use core::error::Error as CoreError;

pub use vpratt_macros::parser;
pub use vpratt_macros::handler;

/// The numerical weight used to determine the Left Binding Power (LBP) of operators.
/// Higher numbers bind tighter (e.g., `*` might be 40, `+` might be 20).
pub type Precedence = u16;

/// The base mechanical errors that the `vpratt` engine can encounter during parsing.
///
/// If a user provides a custom error type via `Result<Expr, MyError>`, the `#[vpratt::parser]`
/// macro will expect the user to implement `From<VprattError> for MyError` to bridge
/// these engine failures into their custom domain.
#[derive(Debug, Clone, PartialEq)]
pub enum VprattError<Token, PrattToken> {
    UnexpectedEOF,
    UnexpectedToken(Token),
    UnmatchedDelimiter(PrattToken, Token),
    ExpectedTokenMismatch(PrattToken, Token),
}

impl<Token: Debug, PrattToken: Debug> Display for VprattError<Token, PrattToken> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnexpectedEOF => write!(f, "Syntax error: Unexpected end of file"),
            Self::UnexpectedToken(tok) => write!(f, "Syntax error: Unexpected token {:?}", tok),
            Self::UnmatchedDelimiter(exp, found) => {
                write!(f, "Syntax error: Expected closing delimiter {:?}, but found {:?}", exp, found)
            }
            Self::ExpectedTokenMismatch(exp, found) => {
                write!(f, "Syntax error: Expected {:?}, but found {:?}", exp, found)
            }
        }
    }
}

impl<Token: Debug, PrattToken: Debug> CoreError for VprattError<Token, PrattToken> {}

pub type Result<P> = core::result::Result<
    <P as crate::VprattCore>::Output,
    <P as crate::VprattCore>::Error
>;

/// The internal trait powering the Left Binding Power (LBP) loop.
///
/// This trait is automatically implemented for your parser struct by the `#[parser]` attribute macro.
/// It contains the hidden state mechanisms required to drive Pratt parsing without allocating.
pub trait VprattCore {
    /// The raw token type yielded by the supplied stream iterator.
    type Item;

    /// The simplified routing token (usually a `TokenKind` enum) extracted from the `Item`.
    type PrattToken: Clone + Copy + PartialEq;

    /// The Abstract Syntax Tree (AST) node type returned.
    type Output;

    /// The error type returned by the handlers.
    type Error;
    
    #[doc(hidden)]
    fn __next__(&mut self) -> Option<Self::Item>;
    #[doc(hidden)]
    fn __peek__(&mut self) -> Option<&Self::Item>;

    #[doc(hidden)]
    fn __extract__(token: &Self::Item) -> Self::PrattToken;
    #[doc(hidden)]
    fn __lbp__(token: &Self::Item) -> Precedence;

    /// The Null Denotation (NUD) router. Handles prefix expressions (numbers, variables, `-x`).
    #[doc(hidden)]
    fn __nud__(&mut self, token: Self::Item) -> Result<Self>;

    /// The Left Denotation (LED) router. Handles infix expressions (`+`, `*`, `^`).
    #[doc(hidden)]
    fn __led__(&mut self, lhs: Self::Output, token: Self::Item) -> Result<Self>;

    #[doc(hidden)]
    fn __convert_error__(err: VprattError<Self::Item, Self::PrattToken>) -> Self::Error;

    /// The core Pratt parsing algorithm.
    /// Consumes a prefix token, routes it, and then yields to the infix loop.
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

    /// The Left Binding Power (LBP) loop.
    /// Continuously consumes tokens as long as their binding power is strictly greater than the current context.
    #[doc(hidden)]
    fn __pratt_resume_infix__(&mut self, mut lhs: Self::Output, rbp: Precedence) -> Result<Self> {
        loop {
            let lbp = match self.__peek__() {
                Some(peeked) => Self::__lbp__(peeked),
                None => break
            };

            if lbp <= rbp { break; }

            // Consume the raw token to pass to the handler
            let token = self.__next__().unwrap();
            lhs = self.__led__(lhs, token)?;
        }
        Ok(lhs)
    }
}

// experimental entry point
// pub trait ParseFromStream<I>: Sized
// where
//     I: Iterator
// {
//     type Parser: VprattCore<Output = Self>;
//     fn parse_stream(
//         stream: I
//     ) -> core::result::Result<Self, <Self::Parser as VprattCore>::Error>;
// }

/// Defines how operators of the same precedence bind to each other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    /// `a - b - c` binds as `(a - b) - c`
    Left,
    /// `a ^ b ^ c` binds as `a ^ (b ^ c)`
    Right
}

/// A wrapper proving that a specific token was consumed by the engine and is ready for use.
#[derive(Debug, Clone, Copy)]
pub struct Consumed<T> {
    pub token: T
}

impl<T> Consumed<T> {
    #[doc(hidden)]
    pub fn __new__(token: T) -> Self { Self { token } }
}


#[derive(Debug, Clone, Copy)]
pub struct Atom<P: VprattCore> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Atom<P> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn __new__() -> Self { Self { _marker: PhantomData } }

    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> Result<P> {
        let token = p.__next__().ok_or_else(|| P::__convert_error__(VprattError::UnexpectedEOF))?;
        p.__nud__(token)
    }
}


/// A capability token that delegates a consumed token back to the prefix routing table,
/// starting a completely fresh expression evaluation.
#[derive(Debug, Clone, Copy)]
pub struct Seed<P: VprattCore> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Seed<P> {
    #[doc(hidden)]
    #[inline(always)]
    pub fn __new__() -> Self { Self { _marker: PhantomData } }

    #[inline(always)]
    pub fn parse(&self, p: &mut P, token: P::Item) -> Result<P> {
        p.__nud__(token)
    }
}

/// A capability token allowing a handler to request the parsing of the right-hand side of an expression.
///
/// This token safely encapsulates the required Left Binding Power (LBP) context. When you call `.parse()`,
/// the engine guarantees it will only consume tokens that bind tighter than the operator that spawned this `Rhs`.
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
    pub fn parse(&self, p: &mut P) -> Result<P> {
        p.__pratt_parse_internal__(self.rbp)
    }
}

/// A capability token that temporarily drops precedence context to 0.
///
/// Useful for situations where you want to start a completely fresh parsing sequence
/// from the current position in the token stream, ignoring any previous binding powers.
#[derive(Debug, Clone, Copy)]
pub struct Subexpr<P: VprattCore> {
    pub _marker: PhantomData<P>
}

impl<P: VprattCore> Subexpr<P> {
    #[doc(hidden)]
    pub fn __new__() -> Self { Subexpr { _marker: PhantomData } }

    /// Executes the Pratt loop from a baseline precedence of 0.
    #[inline(always)]
    pub fn parse(&self, p: &mut P) -> Result<P> {
        p.__pratt_parse_internal__(0)
    }
}

/// A capability token that allows a handler to manually reinject an AST node back into the LBP loop.
///
/// This is the primitive that enables native juxtaposition (e.g., implicit multiplication like `2x`).
/// It allows an operator to partially parse a right-hand node, let trailing operators (like `^2`) bind to it,
/// and *then* collapse the implicit multiplication.
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
    pub fn parse(&self, p: &mut P, lhs: P::Output) -> Result<P> {
        p.__pratt_resume_infix__(lhs, self.rbp)
    }
}

/// A capability token for parsing bounded expressions (e.g., parentheses `( ... )`).
///
/// This token temporarily drops precedence to 0 to parse the interior of the bounds,
/// and then mathematically verifies that the next token in the stream matches the `expected_close` delimiter.
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


/// The compile-time routing map for the Pratt parser.
///
/// This struct is purely declarative. By calling its builder methods (`.terminal()`, `.infix()`, etc.),
/// you define the grammatical rules of your language. The `#[parser]` macro reads this static definition
/// and writes the highly optimized LBP loops and match statements for you.
pub struct Table<P> {
    _marker: PhantomData<P>,
}

impl<P: VprattCore> Table<P> {
    /// Initializes a new routing table.
    pub const fn new() -> Self {
        Self { _marker: PhantomData }
    }

    /// Maps a token that represents a standalone value (e.g., numbers, variables, booleans).
    pub const fn terminal(
        self,
        _token: P::PrattToken,
        _handler: fn(&mut P, TerminalCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a token that opens an enclosed group (e.g., standard parentheses `(a + b)`).
    pub const fn group(
        self,
        _open: P::PrattToken,
        _close: P::PrattToken,
        _handler: fn(&mut P, GroupCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a prefix operator (e.g., the unary minus in `-5` or logical NOT in `!true`).
    pub const fn prefix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, PrefixCtx<P>) -> Result<P>
    ) -> Self { self }

    pub const fn structural(
        self,
        _token: P::PrattToken,
        _handler: fn(&mut P, StructuralCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a standard binary infix operator (e.g., `+`, `-`, `*`, `/`, `^`).
    pub const fn infix(
        self,
        _bp: Precedence,
        _assoc: Associativity,
        _token: P::PrattToken,
        _handler: fn(&mut P,InfixCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a postfix operator (e.g., the factorial symbol in `5!`).
    pub const fn postfix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, PostfixCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a grouped juxtaposition operation (e.g., implicit multiplication via parentheses like `2(a+b)`).
    ///
    /// This provides both an `Enclosed` capability to safely parse the interior, and a `Resume` capability
    /// to reinject the result into the LBP loop.
    pub const fn juxt(
        self,
        _bp: Precedence,
        _open: P::PrattToken,
        _close: P::PrattToken,
        _handler: fn(&mut P, JuxtCtx<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a terminal juxtaposition operation (e.g., implicit multiplication via variables like `2x`).
    ///
    /// The `Resume` capability allows the trailing token to undergo operations (like `x^2`) before
    /// the implicit multiplication collapses the nodes.
    pub const fn implied(
        self,
        _bp: Precedence,
        _assoc: Associativity,
        _token: P::PrattToken,
        _handler: fn(&mut P, ImpliedCtx<P>) -> Result<P>
    ) -> Self { self }
}

/// Context provided to handlers registered via `.terminal()`.
///
/// This context represents a standalone value that does not require further mathematical
/// evaluation. It provides the consumed token but contains no capabilities for further
/// parsing.
///
/// **When to use this:**
/// For atomic values such as integers, floats, booleans, and standalone identifiers.
pub struct TerminalCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.group()`.
///
/// This context represents an isolated expression wrapped in matching delimiters.
/// It temporarily drops the engine's precedence to 0 to parse the interior, and
/// guarantees the closing delimiter is found and consumed.
///
/// **When to use this:**
/// For parentheses `(...)`, array declarations `[...]`, or any other balanced delimiters.
pub struct GroupCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub enclosed: Enclosed<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.structural()`.
///
/// This context is the escape hatch from the math engine. It represents control-flow
/// or structural keywords that break standard Pratt precedence loops.
///
/// It provides two specific capabilities:
/// * `atom`: Fetches a single, isolated identifier or terminal without triggering math.
/// * `sub`: Resets the engine's precedence to 0, evaluating a completely fresh block of code.
///
/// **When to use this:**
/// For language constructs like `let`, `if`, `match`, or `while` statements.
pub struct StructuralCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub atom:    Atom<P>,
    pub sub:     Subexpr<P>,
    pub _marker: PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.prefix()`.
///
/// This context represents an operator that precedes an expression. It provides the
/// `Rhs` capability, which instructs the engine to evaluate the right-hand side
/// of the expression using the operator's registered binding power.
///
/// **When to use this:**
/// For unary math or logic operators like `-x`, `!y`, or `~z`.
pub struct PrefixCtx<'a, P: VprattCore> {
    pub consumed: Consumed<P::Item>,
    pub rhs:      Rhs<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.infix()`.
///
/// This context represents a standard binary operator sitting between two expressions.
/// It provides the already-parsed left-hand side (`lhs`) and the `Rhs` capability to
/// evaluate the right-hand side using the operator's registered binding power.
///
/// **When to use this:**
/// For standard binary operations like `+`, `-`, `*`, `/`, or assignment `<-`.
pub struct InfixCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub rhs:      Rhs<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.postfix()`.
///
/// This context represents a trailing operator. It provides the parsed left-hand
/// side (`lhs`) and the consumed token. Because the expression ends here, it does
/// not provide any capabilities to parse further.
///
/// **When to use this:**
/// For trailing operations like factorial `x!`, increments `y++`, or unwraps `z?`.
pub struct PostfixCtx<'a, P: VprattCore> {
    pub  lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.juxt()`.
///
/// This context represents implicit juxtaposition involving an enclosed group.
/// It provides the `Enclosed` capability to safely parse the interior of the group,
/// and the `Resume` capability to manually reinject the resulting AST node back
/// into the left-binding-power loop.
///
/// **When to use this:**
/// For implicit multiplication using parentheses, e.g., `2(a + b)`.
pub struct JuxtCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub enclosed: Enclosed<P>,
    pub resume:   Resume<P>,
    pub _marker:  PhantomData<&'a ()>,
}

/// Context provided to handlers registered via `.implied()`.
///
/// This context represents pure, token-driven implicit juxtaposition.
/// It provides the `Seed` capability, which forcefully feeds the consumed token
/// back into the prefix router as the start of a new expression, and the `Resume`
/// capability to reinject the combined result back into the infix loop.
///
/// **When to use this:**
/// For ML-style curried function application (e.g., `f x y`) or variable-based
/// implicit multiplication (e.g., `2x`).
pub struct ImpliedCtx<'a, P: VprattCore> {
    pub lhs:      P::Output,
    pub consumed: Consumed<P::Item>,
    pub seed:     Seed<P>,
    pub resume:   Resume<P>,
    pub _marker:  PhantomData<&'a ()>,
}


/// Utility methods available on all vpratt context objects.
///
/// These extend vpratt's correctness guarantees into structural parsing —
/// the parts of a grammar that aren't precedence-driven but still need
/// safe, well-typed stream navigation.
///
/// All methods take `&self` and `p: &mut P` explicitly. The context object
/// is the access point; all stream mutation goes through `p`.
///
/// # Usage
///
/// ```rust
/// ctx.expect(self, Then?;
/// ctx.accept(self, Recursive?;
/// ctx.series(self, Equals?;
/// ctx.separated(self, Comma, RParen?;
/// ```
pub trait CtxUtils<P: VprattCore> {
    /// Consume the next token if it matches `expected`, otherwise return a
    /// typed error.
    ///
    /// Equivalent to "the next token must be X". Surfaces
    /// `VprattError::ExpectedTokenMismatch` if the token doesn't match,
    /// and `VprattError::UnexpectedEOF` if the stream is exhausted.
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
                    // Consume the token so the error carries it.
                    let actual = p.__next__().unwrap();
                    Err(P::__convert_error__(VprattError::ExpectedTokenMismatch(
                        expected,
                        actual,
                    )))
                }
            }
        }
    }

    /// Consume the next token if it matches `expected`, otherwise leave the
    /// stream untouched and return `None`.
    ///
    /// Equivalent to "optionally consume X". Never fails — the `Result`
    /// wrapper exists solely to allow `?` propagation of any internal
    /// stream errors.
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

    /// Consume a sequence of tokens matching `of` until `delimiter` is seen.
    ///
    /// The delimiter is **not** consumed — it is left in the stream for the
    /// caller to handle. Returns an empty `Vec` if the first token is already
    /// the delimiter or a non-matching token.
    ///
    /// # Example
    ///
    /// Parsing `f x y z =` where `=` is the delimiter:
    /// ```rust
    /// // stream: [Ident("x"), Ident("y"), Ident("z"), Equals, ...]
    /// let args = ctx.series(self, Ident(""), Equals)?;
    /// // args: [Consumed(x), Consumed(y), Consumed(z)]
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
                // Delimiter reached — stop, leave it in the stream.
                Some(item) if P::__extract__(item) == delimiter => break,
                // EOF — stop, caller decides if this is an error.
                None => break,
                // Matching token — consume it.
                Some(item) if P::__extract__(item) == of => {
                    let token = p.__next__().unwrap();
                    items.push(Consumed::__new__(token));
                }
                // Non-matching, non-delimiter token — stop.
                _ => break,
            }
        }
        Ok(items)
    }

    /// Consume tokens matching `of`, separated by `sep`, until `end` is seen.
    ///
    /// The `end` delimiter **is** consumed. Leading and trailing separators
    /// are not accepted — the structure must be `of (sep of)* end`.
    /// Returns `VprattError::UnexpectedEOF` if the stream is exhausted before
    /// `end` is seen.
    ///
    /// # Example
    ///
    /// Parsing `(x, y, z)` after `(` has been consumed:
    /// ```rust
    /// // stream: [Ident("x"), Comma, Ident("y"), Comma, Ident("z"), RParen, ...]
    /// let idents = ctx.separated(self, Ident(""), Comma, RParen)?;
    /// // idents: [Consumed(x), Consumed(y), Consumed(z)]
    /// // stream: [...]  (RParen consumed)
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
                // End delimiter reached — consume it and stop.
                Some(item) if P::__extract__(item) == end => {
                    p.__next__();
                    break;
                }
                // EOF before end delimiter — structural error.
                None => {
                    return Err(P::__convert_error__(VprattError::UnexpectedEOF));
                }
                // Expected token — consume it.
                Some(item) if P::__extract__(item) == of => {
                    let token = p.__next__().unwrap();
                    items.push(Consumed::__new__(token));
                }
                // Anything else after we have at least one item is an error.
                _ => {
                    let actual = p.__next__().unwrap();
                    return Err(P::__convert_error__(VprattError::UnexpectedToken(actual)));
                }
            }

            // After each item, expect either a separator or the end delimiter.
            match p.__peek__() {
                Some(item) if P::__extract__(item) == sep => {
                    p.__next__(); // consume separator, continue
                }
                Some(item) if P::__extract__(item) == end => {
                    p.__next__(); // consume end delimiter, stop
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

impl<'a, P: VprattCore> CtxUtils<P> for crate::TerminalCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::GroupCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::StructuralCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::PrefixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::InfixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::PostfixCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::ImpliedCtx<'a, P> {}
impl<'a, P: VprattCore> CtxUtils<P> for crate::JuxtCtx<'a, P> {}