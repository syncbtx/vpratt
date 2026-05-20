use proc_macro2::TokenStream;
use quote::quote;
use syn::{GenericArgument, ImplItem, ItemImpl, PathArguments, ReturnType, Type};
use crate::args::ParserConfig;
use crate::table::{Assoc, Entry, TableDef};

pub fn generate_core(config: &ParserConfig, table: &TableDef, impl_block: &ItemImpl) -> syn::Result<TokenStream> {
    let self_ty = &impl_block.self_ty;
    let (impl_generics, _, where_clause) = impl_block.generics.split_for_impl();

    let (output_ty, inferred_error_type) = infer_types_from_handlers(impl_block)?;

    let entry_method = config.entry.as_ref().map(|i| quote! { #i }).unwrap_or_else(|| quote! { pratt_parse });
    let token_ty = config.token.as_ref().map(|t| quote! { #t }).unwrap_or_else(|| quote! { Self::Item });
    let extractor = config.extract.as_ref().map(|e| quote! { #e }).unwrap_or_else(|| quote! { |t| t.clone() });
    let stream_expr = &config.stream;

    let (error_type, convert_impl) = match inferred_error_type.as_ref() {
        Some(custom_err) => (
            quote! { #custom_err },
            quote! { err.into() }
        ),
        None => (
            quote! { ::vpratt::VprattError<Self::Item, Self::PrattToken> },
            quote! { err }
        ),
    };

    let make_call = |call: TokenStream| {
        if inferred_error_type.is_none() {
            quote! { Ok(#call) }
        } else {
            call
        }
    };

    // 5. Generate the NUD and LED Match Arms
    let mut nud_arms = Vec::new();
    let mut led_arms = Vec::new();
    let mut lbp_arms = Vec::new();

    for entry in &table.entries {
        match entry {
            Entry::Terminal { token, handler, .. } => {
                let call = make_call(quote! { #handler(self, ::vpratt::Consumed::__new__(__token__)) });
                nud_arms.push(quote! { #token => #call, });
            }
            Entry::Group { open, close, handler, .. } => {
                let call = make_call(quote! { #handler(self, ::vpratt::Consumed::__new__(__token__), ::vpratt::Enclosed::__new__(#close)) });
                nud_arms.push(quote! { #open => #call, });
            }
            Entry::Prefix { bp, token, handler, .. } => {
                let call = make_call(quote! { #handler(self, ::vpratt::Consumed::__new__(__token__), ::vpratt::Rhs::__new__(#bp)) });
                nud_arms.push(quote! { #token => #call, });
            }
            Entry::Infix { bp, assoc, token, handler, .. } => {
                let rbp = if *assoc == Assoc::Left { quote! { #bp } } else { quote! { #bp - 1 } };
                lbp_arms.push(quote! { #token => #bp, });
                let call = make_call(quote! { #handler(self, __lhs__, ::vpratt::Consumed::__new__(__token__), ::vpratt::Rhs::__new__(#rbp)) });
                led_arms.push(quote! { #token => #call, });
            }
            Entry::Postfix { bp, token, handler, .. } => {
                lbp_arms.push(quote! { #token => #bp, });
                let call = make_call(quote! { #handler(self, __lhs__, ::vpratt::Consumed::__new__(__token__)) });
                led_arms.push(quote! { #token => #call, });
            }
            Entry::Juxt { bp, open, close, handler, .. } => {
                lbp_arms.push(quote! { #open => #bp, });
                let call = make_call(quote! { #handler(self, __lhs__, ::vpratt::Consumed::__new__(__token__), ::vpratt::Enclosed::__new__(#close), ::vpratt::Resume::__new__(#bp)) });
                led_arms.push(quote! { #open => #call, });
            }
            Entry::Implied { bp, assoc, token, handler, .. } => {
                let rbp = if *assoc == Assoc::Left { quote! { #bp } } else { quote! { #bp - 1 } };
                lbp_arms.push(quote! { #token => #bp, });
                let call = make_call(quote! { #handler(self, __lhs__, ::vpratt::Consumed::__new__(__token__), ::vpratt::Resume::__new__(#rbp)) });
                led_arms.push(quote! { #token => #call, });
            }
        }
    }

    Ok(quote! {
        impl #impl_generics ::vpratt::VprattCore for #self_ty #where_clause {
            type Item = <std::iter::Peekable<I> as std::iter::Iterator>::Item;
            type PrattToken = #token_ty;
            type Output = #output_ty;

            // Injected dynamic error type
            type Error = #error_type;

            #[inline(always)]
            fn __convert_error__(err: ::vpratt::VprattError<Self::Item, Self::PrattToken>) -> Self::Error {
                #convert_impl
            }

            #[inline(always)]
            fn __extract__(__token__: &Self::Item) -> Self::PrattToken {
                let __extractor__ = #extractor;
                __extractor__(__token__)
            }

            #[inline(always)] fn __next__(&mut self) -> Option<Self::Item> { #stream_expr.next() }
            #[inline(always)] fn __peek__(&mut self) -> Option<&Self::Item> { #stream_expr.peek() }

            #[inline(always)]
            fn __lbp__(__token__: &Self::Item) -> ::vpratt::Precedence {
                match Self::__extract__(__token__) {
                    #( #lbp_arms )*
                    _ => 0,
                }
            }

            #[inline(always)]
            fn __nud__(&mut self, __token__: Self::Item) -> Result<Self::Output, Self::Error> {
                match Self::__extract__(&__token__) {
                    #( #nud_arms )*

                    // Route to the new engine-level error conversion
                    _ => Err(Self::__convert_error__(::vpratt::VprattError::UnexpectedToken(__token__))),
                }
            }

            #[inline(always)]
            fn __led__(&mut self, __lhs__: Self::Output, __token__: Self::Item) -> Result<Self::Output, Self::Error> {
                match Self::__extract__(&__token__) {
                    #( #led_arms )*
                    _ => unreachable!("vpratt engine fault: unmapped LED token"),
                }
            }
        }

        // Generate the public entry point wrapper
        impl #impl_generics #self_ty #where_clause {
            pub fn #entry_method(&mut self) -> Result<#output_ty, #error_type> {
                <Self as ::vpratt::VprattCore>::__pratt_parse_internal__(self, 0)
            }
        }
    })
}

fn infer_types_from_handlers(impl_block: &ItemImpl) -> syn::Result<(Type, Option<Type>)> {
    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            // Check for both `#[vpratt::handler]` and `#[handler]`
            let is_tagged = method.attrs.iter().any(|attr| {
                let path = attr.path();
                let segs = &path.segments;

                (segs.len() == 2 && segs[0].ident == "vpratt" && segs[1].ident == "handler")
                    || (segs.len() == 1 && segs[0].ident == "handler")
            });

            if is_tagged {
                match &method.sig.output {
                    ReturnType::Type(_, ty) => {
                        if let Type::Path(type_path) = &**ty {
                            let last_segment = type_path.path.segments.last().unwrap();
                            if last_segment.ident == "Result" {
                                if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                                    let mut args_iter = args.args.iter();
                                    if let (Some(GenericArgument::Type(ok_ty)), Some(GenericArgument::Type(err_ty))) = (args_iter.next(), args_iter.next()) {
                                        return Ok((ok_ty.clone(), Some(err_ty.clone())));
                                    }
                                }
                            }
                        }
                        return Ok((*ty.clone(), None));
                    }
                    _ => {}
                }
            }
        }
    }
    Err(syn::Error::new_spanned(impl_block, "Could not infer types. Ensure at least one handler method is tagged with `#[vpratt::handler]`."))
}