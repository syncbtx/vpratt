use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, MetaNameValue, Token, Type};
use syn::punctuated::Punctuated;

/// Represents the `#[vpratt::parser(...)]` arguments
pub struct ParserConfig {
    // Required
    pub stream: Expr,

    // Explicit Type Definitions
    pub item: Option<Type>,    // e.g., Token<'a>
    pub token: Option<Type>,   // e.g., TokenKind
    pub output: Option<Type>,  // e.g., Expr
    pub error: Option<Type>,   // e.g., CalcError (defaults to vpratt::VprattError)

    // Logic Callbacks
    pub extract: Option<Expr>, // e.g., |t| t.kind or extract_kind_fn
    pub entry: Option<Ident>,  // e.g., parse_full_expression
    pub table: Option<Expr>,   // e.g., Self::TABLE
}

impl Parse for ParserConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut stream = None;
        let mut item = None;
        let mut token = None;
        let mut output = None;
        let mut error = None;
        let mut extract = None;
        let mut entry = None;
        let mut table = None;

        let args = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?;

        for arg in args {
            let key = arg.path.get_ident()
                .ok_or_else(|| syn::Error::new_spanned(&arg.path, "Expected identifier"))?
                .to_string();

            // We parse the right-hand side as an expression first
            let value = syn::parse2::<Expr>(arg.value.to_token_stream())?;

            match key.as_str() {
                "stream"  => stream = Some(value),

                // For types and idents, we convert the Expr's token stream back into the correct syn AST node
                "item"    => item = Some(syn::parse2(value.to_token_stream())?),
                "token"   => token = Some(syn::parse2(value.to_token_stream())?),
                "output"  => output = Some(syn::parse2(value.to_token_stream())?),
                "error"   => error = Some(syn::parse2(value.to_token_stream())?),
                "entry"   => entry = Some(syn::parse2(value.to_token_stream())?),

                "extract" => extract = Some(value),
                "table"   => table = Some(value),

                _ => return Err(syn::Error::new_spanned(
                    arg.path,
                    "Unknown parser attribute argument. Expected one of: 'stream', 'item', 'token', 'output', 'error', 'extract', 'entry', 'table'"
                )),
            }
        }

        Ok(ParserConfig {
            stream: stream.ok_or_else(|| syn::Error::new(input.span(), "Missing `stream` argument in #[vpratt::parser]"))?,
            item,
            token,
            output,
            error,
            extract,
            entry,
            table,
        })
    }
}