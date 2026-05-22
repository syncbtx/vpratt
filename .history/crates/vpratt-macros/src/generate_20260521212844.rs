use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemImpl;
use crate::args::ParserConfig;
use crate::table::{Assoc, Entry, TableDef};

/// Converts a token expression into a match pattern.
///
/// Handles:
/// - Enum tuple variants:  `Token::Num(1.0)` → `Token::Num(..)`
/// - Enum struct variants: `Token::Point { x: 0 }` → `Token::Point { .. }`
/// - Unit variants / idents: passed through as-is
///
/// Returns a `syn::Error` for any expression form that cannot be
/// safely converted into a pattern, rather than silently emitting
/// broken code.
fn to_pattern(expr: &syn::Expr) -> syn::Result<TokenStream> {
    match expr {
        syn::Expr::Call(call) => {
            let func = &call.func;
            Ok(quote! { #func(..) })
        }
        syn::Expr::Struct(s) => {
            let path = &s.path;
            Ok(quote! { #path { .. } })
        }
        // Unit variants and plain paths (e.g. `TokenKind::Plus`) pass through.
        syn::Expr::Path(_) => Ok(quote! { #expr }),
        // Anything else is a hard error — we cannot safely produce a match arm.
        other => Err(syn::Error::new_spanned(
            other,
            "vpratt: unsupported token expression. \
             Expected an enum variant call (e.g. `Token::Num(0.0)`), \
             a struct variant (e.g. `Token::Point { x: 0 }`), \
             or a plain path (e.g. `Token::Plus`).",
        )),
    }
}

pub fn generate_core(
    config: &ParserConfig,
    table: &TableDef,
    impl_block: &ItemImpl,
) -> syn::Result<TokenStream> {
    let self_ty = &impl_block.self_ty;
    let (impl_generics, _, where_clause) = impl_block.generics.split_for_impl();

    // ── Required arguments

    let output_ty = config.output.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(
            impl_block,
            "vpratt: missing `output` argument in #[vpratt::parser].\n\
             Add `output = YourAstType` to specify the type your handlers return.",
        )
    })?;

    let item_ty = config.item.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(
            impl_block,
            "vpratt: missing `item` argument in #[vpratt::parser].\n\
             Add `item = YourToken` to specify the Iterator::Item type of your stream.",
        )
    })?;

    let token_ty = config.token.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(
            impl_block,
            "vpratt: missing `token` argument in #[vpratt::parser].\n\
             Add `token = YourTokenKind` to specify the routing token type.",
        )
    })?;


    // Defaults to `pratt_parse` — users can call `self.pratt_parse()` without
    // specifying `entry` in the macro arguments.
    let entry_method = config
        .entry
        .as_ref()
        .map(|i| quote! { #i })
        .unwrap_or_else(|| quote! { pratt_parse });

    // Default extractor clones the item. This assumes `Item: Clone` and that
    // `Item` and `PrattToken` are the same type. If your stream item and routing
    // token are different types you must provide an explicit `extract` argument.
    let extractor = config
        .extract
        .as_ref()
        .map(|e| quote! { #e })
        .unwrap_or_else(|| {
            quote! { |t: &#item_ty| t.clone() }
        });

    let stream_expr = &config.stream;

    // ── Error type resolution
    //
    // Two cases:
    //   1. User provides `error = MyError`: we emit `err.into()` and require
    //      `MyError: From<VprattError<Item, Token>>`.
    //   2. No custom error: the associated type IS `VprattError<Item, Token>`,
    //      so no conversion is needed — emit a direct return.
    let (error_type, convert_impl) = match config.error.as_ref() {
        Some(custom_err) => (
            quote! { #custom_err },
            quote! { err.into() },
        ),
        None => (
            quote! { ::vpratt::VprattError<#item_ty, #token_ty> },
            quote! { err },
        ),
    };

    // ── Build match arms──

    let mut nud_arms = Vec::new();
    let mut led_arms = Vec::new();
    let mut lbp_arms = Vec::new();

    for entry in &table.entries {
        match entry {
            Entry::Terminal { token, handler, .. } => {
                let pat = to_pattern(token)?;
                nud_arms.push(quote! {
                    #pat => #handler(self, ::vpratt::Consumed::__new__(__token__)),
                });
            }

            Entry::Group { open, close, handler, .. } => {
                let open_pat = to_pattern(open)?;
                nud_arms.push(quote! {
                    #open_pat => #handler(
                        self,
                        ::vpratt::Consumed::__new__(__token__),
                        ::vpratt::Enclosed::__new__(#close),
                    ),
                });
            }

            Entry::Prefix { bp, token, handler, .. } => {
                let pat = to_pattern(token)?;
                nud_arms.push(quote! {
                    #pat => #handler(
                        self,
                        ::vpratt::Consumed::__new__(__token__),
                        ::vpratt::Rhs::__new__(#bp),
                    ),
                });
            }

            Entry::Infix { bp, assoc, token, handler, .. } => {
                let pat = to_pattern(token)?;
                let rbp = if *assoc == Assoc::Left {
                    quote! { #bp }
                } else {
                    quote! { #bp - 1 }
                };
                lbp_arms.push(quote! { #pat => #bp, });
                led_arms.push(quote! {
                    #pat => #handler(
                        self,
                        __lhs__,
                        ::vpratt::Consumed::__new__(__token__),
                        ::vpratt::Rhs::__new__(#rbp),
                    ),
                });
            }

            Entry::Postfix { bp, token, handler, .. } => {
                let pat = to_pattern(token)?;
                lbp_arms.push(quote! { #pat => #bp, });
                led_arms.push(quote! {
                    #pat => #handler(self, __lhs__, ::vpratt::Consumed::__new__(__token__)),
                });
            }

            Entry::Juxt { bp, open, close, handler, .. } => {
                let open_pat = to_pattern(open)?;
                lbp_arms.push(quote! { #open_pat => #bp, });
                led_arms.push(quote! {
                    #open_pat => #handler(
                        self,
                        __lhs__,
                        ::vpratt::Consumed::__new__(__token__),
                        ::vpratt::Enclosed::__new__(#close),
                        ::vpratt::Resume::__new__(#bp),
                    ),
                });
            }

            Entry::Implied { bp, assoc, token, handler, .. } => {
                let pat = to_pattern(token)?;
                let rbp = if *assoc == Assoc::Left {
                    quote! { #bp }
                } else {
                    quote! { #bp - 1 }
                };
                lbp_arms.push(quote! { #pat => #bp, });
                led_arms.push(quote! {
                    #pat => #handler(
                        self,
                        __lhs__,
                        ::vpratt::Consumed::__new__(__token__),
                        ::vpratt::Resume::__new__(#rbp),
                    ),
                });
            }
        }
    }


    Ok(quote! {
        impl #impl_generics ::vpratt::VprattCore for #self_ty #where_clause {
            type Item       = #item_ty;
            type PrattToken = #token_ty;
            type Output     = #output_ty;
            type Error      = #error_type;

            /// Converts a `VprattError` into `Self::Error`.
            ///
            /// When no custom `error` argument is provided this is a no-op
            /// identity function and LLVM eliminates it entirely. When a
            /// custom error type is provided it delegates to `Into::into`,
            /// requiring `Self::Error: From<VprattError<Item, Token>>`.
            #[inline(always)]
            fn __convert_error__(
                err: ::vpratt::VprattError<Self::Item, Self::PrattToken>,
            ) -> Self::Error {
                #convert_impl
            }

            /// Extracts the routing `PrattToken` from a stream `Item`.
            ///
            /// The extractor is either the user-supplied `extract` argument
            /// or the default `|t| t.clone()`. It is called once per token
            /// and is always inlined — if it is a function pointer or a
            /// trivial closure LLVM will eliminate the call entirely.
            #[inline(always)]
            fn __extract__(__token__: &Self::Item) -> Self::PrattToken {
                // Binding the extractor to a local ensures both function
                // pointers and closures are called through the same path,
                // giving LLVM a single inlining site regardless of form.
                let __extractor__: fn(&Self::Item) -> Self::PrattToken = #extractor;
                __extractor__(__token__)
            }

            #[inline(always)]
            fn __next__(&mut self) -> Option<Self::Item> {
                #stream_expr.next()
            }

            #[inline(always)]
            fn __peek__(&mut self) -> Option<&Self::Item> {
                #stream_expr.peek()
            }

            #[inline(always)]
            fn __lbp__(__token__: &Self::Item) -> ::vpratt::Precedence {
                match Self::__extract__(__token__) {
                    #( #lbp_arms )*
                    // Tokens not registered as infix/postfix/implied/juxt
                    // have zero binding power — they terminate the current
                    // expression and are left in the stream.
                    _ => 0,
                }
            }

            #[inline(always)]
            fn __nud__(
                &mut self,
                __token__: Self::Item,
            ) -> ::core::result::Result<Self::Output, Self::Error> {
                match Self::__extract__(&__token__) {
                    #( #nud_arms )*
                    _ => Err(Self::__convert_error__(
                        ::vpratt::VprattError::UnexpectedToken(__token__),
                    )),
                }
            }

            #[inline(always)]
            fn __led__(
                &mut self,
                __lhs__: Self::Output,
                __token__: Self::Item,
            ) -> ::core::result::Result<Self::Output, Self::Error> {
                match Self::__extract__(&__token__) {
                    #( #led_arms )*
                    // SAFETY: `__led__` is only called when `__lbp__` returned
                    // a nonzero value for this token. If we reach this arm it
                    // means `__lbp__` and `__led__` are out of sync, which is
                    // a bug in the vpratt macro expansion, not in user code.
                    _ => {
                        #[cfg(debug_assertions)]
                        panic!(
                            "vpratt internal fault: `__led__` received a token \
                             that `__lbp__` assigned nonzero precedence to but \
                             has no corresponding LED arm. This is a bug in \
                             vpratt — please open an issue."
                        );
                        #[cfg(not(debug_assertions))]
                        unsafe { ::core::hint::unreachable_unchecked() }
                    }
                }
            }
        }

        impl #impl_generics #self_ty #where_clause {
            /// Entry point generated by `#[vpratt::parser]`.
            ///
            /// Parses a complete expression from the token stream,
            /// respecting the precedence table defined in `TABLE`.
            pub fn #entry_method(
                &mut self,
            ) -> ::core::result::Result<#output_ty, #error_type> {
                <Self as ::vpratt::VprattCore>::__pratt_parse_internal__(self, 0)
            }
        }

        impl #impl_generics ::vpratt::ParseFromStream for #output_ty{
            type Parser = Self;
            fn parse_stream(stream: impl IntoIterator<Item = <Self::Parser as ::vpratt::VprattCore::Error>>) -> ::vpratt::Result<#self_ty{
                let parser =
            }
        }
    })
}