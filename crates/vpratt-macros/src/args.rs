use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, Token, Type};

pub struct ParserConfig {
    pub stream: Expr,
    pub item: Option<Type>,
    pub token: Option<Type>,
    pub output: Option<Type>,
    pub error: Option<Type>,
    pub extract: Option<Expr>,
    pub entry: Option<Ident>,
    pub _table: Option<Expr>,
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
        let mut _table = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            input.parse::<Token![=]>()?;

            let key_str = key.to_string();
            match key_str.as_str() {
                "item"   => item = Some(input.parse::<Type>()?),
                "token"  => token = Some(input.parse::<Type>()?),
                "output" => output = Some(input.parse::<Type>()?),
                "error"  => error = Some(input.parse::<Type>()?),

                "stream"  => stream = Some(input.parse::<Expr>()?),
                "extract" => extract = Some(input.parse::<Expr>()?),
                "table"   => _table = Some(input.parse::<Expr>()?),

                "entry"  => entry = Some(input.parse::<Ident>()?),

                _ => return Err(syn::Error::new_spanned(
                    key,
                    "Unknown parser attribute argument. Expected one of: 'stream', 'item', 'token', 'output', 'error', 'extract', 'entry', 'table'"
                )),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
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
            _table,
        })
    }
}