## [0.6.0] — 2026-06-01

### Breaking
- Context objects replace flat capability token handler signatures
- `series` and `separated` now return `Vec<P::Output>` instead of `Vec<Consumed<P::Item>>`  
- `CtxUtils` methods now take `p: &mut P` as first argument before token arguments
- `Precedence` is now `u16` by default (opt-in `u8` via `features = ["bp_u8"]`)
- `#[vpratt::token]` macro introduced — `token` and `extract` arguments now optional when used

### Added
- `series_sub` and `separated_sub` on `CtxUtils`
- `VprattToken` trait
- `Spanned` trait
- `#[vpratt::token]` attribute macro
- `mincaml` and `query` examples

### Changed
- README updated
- DOC.md replaced with Reference.md
