//! Error types for the vpratt engine.
//!
//! The engine surfaces exactly four failure modes as variants
//! of [`VprattError`].
//!
//! These map directly onto structural problems in the
//! token stream — nothing more, nothing less.
//!
//! ## Custom error types
//!
//! For production use, provide your own error type via `error = MyError`
//! in `#[vpratt::parser]` and implement `From<VprattError>` to map engine
//! failures into your diagnostic infrastructure:
//!
//! ```rust
//! impl From<VprattError<Token<'_>, TokenKind<'_>>> for Diagnostic<'_> {
//!     fn from(err: VprattError<Token, TokenKind>) -> Self {
//!         match err {
//!             VprattError::UnexpectedEOF                           => Diagnostic::eof(),
//!             VprattError::UnexpectedToken(t)                      => Diagnostic::unexpected(t.span),
//!             VprattError::UnmatchedDelimiter(expected, actual)    => Diagnostic::unmatched(actual.span),
//!             VprattError::ExpectedTokenMismatch(expected, actual) => Diagnostic::expected(expected, actual.span),
//!         }
//!     }
//! }
//! ```

use core::fmt::{Debug, Display, Formatter, Result as FmtResult};
use core::error::Error as CoreError;

/// The failure modes of the vpratt engine.
///
/// Each variant represents a specific structural problem in the token stream.
///
/// When no custom error type is provided, `vpratt::Result<Self>` resolves to
/// `Result<Output, VprattError<Item, PrattToken>>` directly.
///
/// When a custom error type is provided via `error = MyError` in
/// `#[vpratt::parser]`, implement `From<VprattError<Item, PrattToken>> for MyError`
/// to convert engine failures into your own diagnostic type.
///
/// The conversion happens automatically at every error site — you write it once.
///
/// ## Variants
///
/// | Variant | When it fires |
/// |---|---|
/// | [`UnexpectedEOF`] | Stream exhausted while an expression was expected |
/// | [`UnexpectedToken`] | No prefix, terminal, or group rule matched this token |
/// | [`UnmatchedDelimiter`] | A `.group()` or `.juxt()` reached EOF before its closing token |
/// | [`ExpectedTokenMismatch`] | `ctx.expect()` found a different token than requested |
///
/// [`UnexpectedEOF`]: VprattError::UnexpectedEOF
/// [`UnexpectedToken`]: VprattError::UnexpectedToken
/// [`UnmatchedDelimiter`]: VprattError::UnmatchedDelimiter
/// [`ExpectedTokenMismatch`]: VprattError::ExpectedTokenMismatch
#[derive(Debug, Clone, PartialEq)]
pub enum VprattError<Token, PrattToken> {
    /// The token stream was exhausted while the engine expected further input.
    ///
    /// Fired when a prefix token has no right-hand side to consume — for
    /// example a trailing `2 +` or an unclosed structural construct.
    UnexpectedEOF,

    /// A token appeared in the prefix position with no matching rule.
    ///
    /// Carries the offending token. Fired when neither `.terminal()`,
    /// `.prefix()`, `.group()`, nor `.structural()` has a rule for this token
    /// at the current parse position.
    UnexpectedToken(Token),

    /// A `.group()` or `.juxt()` construct reached EOF before its closing delimiter.
    ///
    /// Carries the expected closing token kind and the actual token found
    /// (or the last token before EOF).
    UnmatchedDelimiter(PrattToken, Token),

    /// `ctx.expect()` found a different token than the one requested.
    ///
    /// Carries the expected token kind and the actual token found.
    ///
    /// Fired exclusively by the [`CtxUtils::expect`](crate::CtxUtils::expect)
    /// utility method.
    ExpectedTokenMismatch(PrattToken, Token),
}

impl<Token: Debug, PrattToken: Debug> Display for VprattError<Token, PrattToken> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::UnexpectedEOF => {
                write!(f, "Syntax error: unexpected end of file")
            }
            Self::UnexpectedToken(tok) => {
                write!(f, "Syntax error: unexpected token {:?}", tok)
            }
            Self::UnmatchedDelimiter(expected, found) => {
                write!(
                    f,
                    "Syntax error: expected closing delimiter {:?}, found {:?}",
                    expected, found
                )
            }
            Self::ExpectedTokenMismatch(expected, found) => {
                write!(
                    f,
                    "Syntax error: expected {:?}, found {:?}",
                    expected, found
                )
            }
        }
    }
}

impl<Token: Debug, PrattToken: Debug> CoreError for VprattError<Token, PrattToken> {}