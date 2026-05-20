use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, MetaNameValue, Token, Type};
use syn::punctuated::Punctuated;

/// Represents the `#[vpratt::parser(...)]` arguments
pub struct ParserConfig {
    pub stream: Expr,
    pub entry: Option<Ident>,
    pub token: Option<Type>,
    pub extract: Option<Expr>,
}

impl Parse for ParserConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut stream = None;
        let mut entry = None;
        let mut token = None;
        let mut extract = None;
        // let mut error = None;
        //let mut item = None;

        let args = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?;

        for arg in args {
            let key = arg.path.get_ident()
                .ok_or_else(|| syn::Error::new_spanned(&arg.path, "Expected identifier"))?
                .to_string();

            let value = syn::parse2::<Expr>(arg.value.to_token_stream())?;

            match key.as_str() {
                "stream" => stream = Some(value),
                "entry" => entry = Some(syn::parse2(value.to_token_stream())?),
                "token" => token = Some(syn::parse2(value.to_token_stream())?),
                "extract" => extract = Some(value),
                // "item" => extract = Some(value),
                // "error" => extract = Some(value),
                _ => return Err(syn::Error::new_spanned(arg.path, "Unknown parser attribute argument, expected 'stream | entry | token | extract'")),
            }
        }

        Ok(ParserConfig {
            stream: stream.ok_or_else(|| syn::Error::new(input.span(), "Missing `stream`"))?,
            entry,
            token,
            extract,
        })
    }
}