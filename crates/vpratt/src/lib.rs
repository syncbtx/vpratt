//! A zero-allocation, `no_std` framework for strict O(N) Pratt and Recursive Descent parsing in Rust.
//!
//! ## Architecture
//!
//! `vpratt` is split into two crates:
//!
//! **The Core (this crate)** — mathematical primitives, typed context objects,
//!   and the [`VprattCore`] trait.
//! 
//! **The Macros (`vpratt-macros`)** — reads your [`Table`] definition and
//!   generates the LBP loop and dispatch automatically.
//!
//! ## How it works
//!
//! You declare a [`Table`] mapping tokens to handler functions. 
//! 
//! The format you choose — [`.terminal()`](Table::terminal), [`.infix()`](Table::infix), [`.structural()`](Table::structural) etc. — determines
//! the typed context your handler receives. 
//! 
//! Every context carries exactly the fields its format requires, nothing more.
//!
//! ```rust
//! const TABLE: Table<Self> = Table::new()
//!     .terminal(Num(0.0),      Self::num)
//!     .infix(20, Left, Plus,   Self::add)
//!     .prefix(30, Minus,       Self::negate);
//!
//! #[vpratt::handler]
//! fn add(&mut self, ctx: InfixCtx<Self>) -> vpratt::Result<Self> {
//!     Ok(Expr::Add(Box::new(ctx.lhs), Box::new(ctx.rhs.parse(self)?)))
//! }
//! ```
//!
//! ## Context objects
//!
//! Each format maps to a dedicated context type:
//!
//! | Format | Context |
//! |---|---|
//! | `.terminal()` | [`TerminalCtx`] |
//! | `.group()` | [`GroupCtx`] |
//! | `.prefix()` | [`PrefixCtx`] |
//! | `.structural()` | [`StructuralCtx`] |
//! | `.infix()` | [`InfixCtx`] |
//! | `.postfix()` | [`PostfixCtx`] |
//! | `.implied()` | [`ImpliedCtx`] |
//! | `.juxt()` | [`JuxtCtx`] |
//!
//! ## Capability tokens
//!
//! Capability tokens are fields on context objects. You access them via the
//! context and drive them by calling `.parse(self)`:
//!
//! - `ctx.consumed` — the triggering token
//! - `ctx.rhs` — parse the RHS at the correct binding power
//! - `ctx.sub` — parse a sub-expression from precedence 0
//! - `ctx.atom` — parse a single atomic terminal
//! - `ctx.enclosed` — parse a delimited sub-expression and consume the closing token
//! - `ctx.seed` — re-enter the NUD phase with a pre-consumed token
//! - `ctx.resume` — re-enter the infix loop with a synthetic LHS
//!
//! ## Structural parsing
//!
//! For keyword-driven constructs, [`.structural()`](Table::structural) handlers have access to the
//! [`CtxUtils`] trait methods for safe, typed stream navigation:
//!
//! - `ctx.expect(self, token)?`
//! - `ctx.accept(self, token)?`
//! - `ctx.series(self, of, delimiter)?`
//! - `ctx.separated(self, of, sep, end)?`

#![no_std]
extern crate alloc;

pub mod prelude;
pub mod capabilities;
pub mod core;
pub mod error;
pub mod dsl;
pub mod context;

#[doc(inline)] pub use vpratt_macros::parser;
#[doc(inline)] pub use vpratt_macros::handler;
#[doc(inline)] pub use crate::capabilities::*;
#[doc(inline)] pub use crate::core::*;
#[doc(inline)] pub use crate::context::utils::CtxUtils;
#[doc(inline)] pub use crate::context::{TerminalCtx, GroupCtx, StructuralCtx, PrefixCtx, InfixCtx,PostfixCtx, ImpliedCtx,JuxtCtx};
#[doc(inline)] pub use crate::error::*;
#[doc(inline)] pub use crate::dsl::*;













