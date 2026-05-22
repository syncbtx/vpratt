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

pub mod prelude;

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

pub trait ParseFromStream{
    type Parser: VprattCore<Output = Self>;
}

/// Defines how operators of the same precedence bind to each other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Associativity {
    /// `a - b - c` binds as `(a - b) - c`
    Left,
    /// `a ^ b ^ c` binds as `a ^ (b ^ c)`
    Right
}

/// A wrapper proving that a specific token was consumed by the engine and is ready for use.
pub struct Consumed<T> {
    pub token: T
}

impl<T> Consumed<T> {
    #[doc(hidden)]
    pub fn __new__(token: T) -> Self { Self { token } }
}

/// A capability token allowing a handler to request the parsing of the right-hand side of an expression.
///
/// This token safely encapsulates the required Left Binding Power (LBP) context. When you call `.parse()`,
/// the engine guarantees it will only consume tokens that bind tighter than the operator that spawned this `Rhs`.
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
pub struct Reset<P: VprattCore> {
    pub _marker: PhantomData<P>
}

impl<P: VprattCore> Reset<P> {
    #[doc(hidden)]
    pub fn __new__() -> Self { Reset { _marker: PhantomData } }

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
            Err(P::__convert_error__(VprattError::E(self.expected_close, token)))
        }
    }
}

pub struct Expect<P: VprattCore>{
    pub expected: P::PrattToken,
}

impl<P: VprattCore> Expect<P>  {
    pub fn new(expected: P::PrattToken) -> Self { Self { expected }}
    pub fn parse(&self, parser: &mut P) -> core::result::Result<P::Item, P::Error>{
        let token = match parser.__next__(){
            Some(t) => t,
            None => return Err(P::__convert_error__(VprattError::UnexpectedEOF))
        };

        if self.expected == P::__extract__(&token){
            Ok(token)
        }
        else{
            Err(P::__convert_error__(VprattError::ExpectedTokenMismatch(self.expected, token)))
        }
}

pub struct Accept<P: VprattCore>{
    pub target: P::PrattToken
}

impl<P: VprattCore> Accept<P>{
    pub fn new(target: P::PrattToken) -> Self { Self{ target } }
    pub fn parse(&self, parser: &mut P) -> core::result::Result<core::option::Option<P::Item>>{
         let is_match = match parser.__peek__(){
            Some(t) => P::__extract__(t) == self.target,
            None => false
        };

        if is_match{
            Ok(Some(parser.__next__().unwrap()))
        }
        else{
            Err(None)
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
        _handler: fn(&mut P, Consumed<P::Item>) -> Result<P>
    ) -> Self { self }

    /// Maps a token that opens an enclosed group (e.g., standard parentheses `(a + b)`).
    pub const fn group(
        self,
        _open: P::PrattToken,
        _close: P::PrattToken,
        _handler: fn(&mut P, Consumed<P::Item>, Enclosed<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a prefix operator (e.g., the unary minus in `-5` or logical NOT in `!true`).
    pub const fn prefix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, Consumed<P::Item>, Rhs<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a standard binary infix operator (e.g., `+`, `-`, `*`, `/`, `^`).
    pub const fn infix(
        self,
        _bp: Precedence,
        _assoc: Associativity,
        _token: P::PrattToken,
        _handler: fn(&mut P, P::Output, Consumed<P::Item>, Rhs<P>) -> Result<P>
    ) -> Self { self }

    /// Maps a postfix operator (e.g., the factorial symbol in `5!`).
    pub const fn postfix(
        self,
        _bp: Precedence,
        _token: P::PrattToken,
        _handler: fn(&mut P, P::Output, Consumed<P::Item>) -> Result<P>
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
        _handler: fn(&mut P, P::Output, Consumed<P::Item>, Enclosed<P>, Resume<P>) -> Result<P>
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
        _handler: fn(&mut P, P::Output, Consumed<P::Item>, Resume<P>) -> Result<P>
    ) -> Self { self }
}