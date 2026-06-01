// #[vpratt::token] accepts one or two arguments:
//
//   #[vpratt::token(self)]          — item is its own routing token
//   #[vpratt::token(kind)]          — named field for routing token
//   #[vpratt::token(0)]             — tuple index for routing token
//   #[vpratt::token(self, span)]    — self + named span field
//   #[vpratt::token(kind, span)]    — named routing + named span field
//   #[vpratt::token(0, 1)]          — tuple index routing + tuple index span

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, ItemType, Fields};
use syn::parse::Parser;

pub enum TokenField {
    Selff,              // `self`
    Named(syn::Ident),  // `kind`
    Index(syn::Index),  // `0`
}

pub struct TokenArgs {
    pub routing: TokenField,
    pub span:    Option<TokenField>,
}

pub fn parse_args(args: TokenStream) -> syn::Result<TokenArgs> {
    let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let exprs  = parser.parse2(args.clone())?;

    if exprs.is_empty() || exprs.len() > 2 {
        return Err(syn::Error::new_spanned(
            args,
            "#[vpratt::token] requires one or two arguments.\n\
             Examples:\n\
             \t#[vpratt::token(self)]\n\
             \t#[vpratt::token(kind)]\n\
             \t#[vpratt::token(kind, span)]",
        ));
    }

    let routing = parse_field(&exprs[0])?;
    let span    = exprs.get(1).map(parse_field).transpose()?;

    Ok(TokenArgs { routing, span })
}

fn parse_field(expr: &syn::Expr) -> syn::Result<TokenField> {
    // `self` keyword
    if let syn::Expr::Path(p) = expr {
        if p.path.is_ident("self") {
            return Ok(TokenField::Selff);
        }
        // named identifier
        if let Some(ident) = p.path.get_ident() {
            return Ok(TokenField::Named(ident.clone()));
        }
    }
    // integer literal — tuple index
    if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit), .. }) = expr {
        let index = lit.base10_parse::<usize>()?;
        return Ok(TokenField::Index(syn::Index::from(index)));
    }
    Err(syn::Error::new_spanned(
        expr,
        "expected `self`, a field name (e.g. `kind`), or a tuple index (e.g. `0`)",
    ))
}


pub fn expand(
    args: TokenStream,
    input: TokenStream,
) -> syn::Result<TokenStream> {
    let token_args = parse_args(args)?;

    if let Ok(item) = syn::parse2::<DeriveInput>(input.clone()) {
        expand_derive_input(token_args, item)
    } else if let Ok(item) = syn::parse2::<ItemType>(input.clone()) {
        expand_type_alias(token_args, item)
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[vpratt::token] can only be applied to a struct, enum, or type alias",
        ))
    }
}

fn expand_derive_input(
    args: TokenArgs,
    item: DeriveInput,
) -> syn::Result<TokenStream> {
    let ident    = &item.ident;
    let generics = &item.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty  = quote! { #ident #ty_generics };

    let (pratt_token_ty, extract_body) = match &args.routing {
        TokenField::Selff => (
            quote! { #self_ty },
            quote! { item.clone() },
        ),
        TokenField::Named(field) => {
            let field_ty = named_field_type(&item, field)?;
            (
                quote! { #field_ty },
                quote! { item.#field.clone() },
            )
        }
        TokenField::Index(index) => {
            let field_ty = tuple_field_type(&item, index)?;
            (
                quote! { #field_ty },
                quote! { item.#index.clone() },
            )
        }
    };

    let vpratt_token_impl = quote! {
        impl #impl_generics ::vpratt::VprattToken for #self_ty #where_clause {
            type Item       = #self_ty;
            type PrattToken = #pratt_token_ty;

            #[inline(always)]
            fn extract(item: &Self::Item) -> Self::PrattToken {
                #extract_body
            }
        }
    };

    let spanned_impl = match &args.span {
        None => quote! {},
        Some(TokenField::Selff) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "span field cannot be `self` — provide a field name or tuple index",
            ));
        }
        Some(TokenField::Named(field)) => {
            let span_ty = named_field_type(&item, field)?;
            quote! {
                impl #impl_generics ::vpratt::Spanned for #self_ty #where_clause {
                    type Span = #span_ty;
                    #[inline(always)]
                    fn span(&self) -> Self::Span { self.#field }
                }
            }
        }
        Some(TokenField::Index(index)) => {
            let span_ty = tuple_field_type(&item, index)?;
            quote! {
                impl #impl_generics ::vpratt::Spanned for #self_ty #where_clause {
                    type Span = #span_ty;
                    #[inline(always)]
                    fn span(&self) -> Self::Span { self.#index }
                }
            }
        }
    };

    Ok(quote! {
        #item
        #vpratt_token_impl
        #spanned_impl
    })
}

fn expand_type_alias(
    args: TokenArgs,
    item: ItemType,
) -> syn::Result<TokenStream> {
    let ident    = &item.ident;
    let generics = &item.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty  = quote! { #ident #ty_generics };

    // For type aliases the macro cannot inspect field types at expansion time.
    // The user is responsible for ensuring the routing and span types are correct.
    // We emit the impls trusting the index/field access — the type checker validates.

    let (pratt_token_ty, extract_body) = match &args.routing {
        TokenField::Selff => (
            quote! { #self_ty },
            quote! { item.clone() },
        ),
        TokenField::Named(field) => {
            return Err(syn::Error::new_spanned(
                field,
                "named field extraction is not supported on type aliases; \
                 use a tuple index instead (e.g. `#[vpratt::token(0)]`)",
            ));
        }
        TokenField::Index(index) => (
            // Type unknown at expansion time — emit a placeholder that the
            // type checker will resolve. User must ensure the index is correct.
            quote! { _ },
            quote! { item.#index.clone() },
        ),
    };

    let vpratt_token_impl = quote! {
        impl #impl_generics ::vpratt::VprattToken for #self_ty #where_clause {
            type Item       = #self_ty;
            type PrattToken = #pratt_token_ty;

            #[inline(always)]
            fn extract(item: &Self::Item) -> Self::PrattToken {
                #extract_body
            }
        }
    };

    let spanned_impl = match &args.span {
        None => quote! {},
        Some(TokenField::Named(field)) => {
            return Err(syn::Error::new_spanned(
                field,
                "named field extraction is not supported on type aliases; \
                 use a tuple index instead (e.g. `#[vpratt::token(0, 1)]`)",
            ));
        }
        Some(TokenField::Selff) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "span field cannot be `self`",
            ));
        }
        Some(TokenField::Index(index)) => quote! {
            impl #impl_generics ::vpratt::Spanned for #self_ty #where_clause {
                type Span = _;
                #[inline(always)]
                fn span(&self) -> Self::Span { self.#index }
            }
        },
    };

    Ok(quote! {
        #item
        #vpratt_token_impl
        #spanned_impl
    })
}

fn named_field_type(
    item: &DeriveInput,
    field: &syn::Ident,
) -> syn::Result<syn::Type> {
    if let syn::Data::Struct(data) = &item.data {
        if let Fields::Named(fields) = &data.fields {
            for f in &fields.named {
                if f.ident.as_ref() == Some(field) {
                    return Ok(f.ty.clone());
                }
            }
        }
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "field `{}` not found on this type — check the field name",
            field
        ),
    ))
}

fn tuple_field_type(
    item: &DeriveInput,
    index: &syn::Index,
) -> syn::Result<syn::Type> {
    if let syn::Data::Struct(data) = &item.data {
        if let Fields::Unnamed(fields) = &data.fields {
            if let Some(field) = fields.unnamed.iter().nth(index.index as usize) {
                return Ok(field.ty.clone());
            }
        }
    }
    Err(syn::Error::new_spanned(
        proc_macro2::Literal::usize_unsuffixed(index.index as usize),
        format!(
            "tuple field `{}` not found on this type — check the index",
            index.index
        ),
    ))
}

