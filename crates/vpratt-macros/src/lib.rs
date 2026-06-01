use proc_macro::TokenStream;

mod args;
mod scrape;
mod table;
mod validate;
mod generate;
mod token;

#[proc_macro_attribute]
pub fn parser(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 1. Parse configs and the block
    let config = syn::parse_macro_input!(attr as args::ParserConfig);
    let mut impl_block = syn::parse_macro_input!(item as syn::ItemImpl);

    // 2. Scrape the AST into the IR
    let table_def = match scrape::scrape_table(&mut impl_block) {
        Ok(def) => def,
        Err(e) => return e.to_compile_error().into(),
    };

    // 3. Validate math
    if let Err(e) = validate::validate(&table_def) {
        return e.to_compile_error().into();
    }

    // 4. Generate the VprattCore trait implementation
    let trait_impl = match generate::generate_core(&config, &table_def, &impl_block) {
        Ok(code) => code,
        Err(e) => return e.to_compile_error().into(),
    };

    // 5. Emit the original block and the generated trait
    let final_code = quote::quote! {
        #impl_block
        #trait_impl
    };

    final_code.into()
}

#[proc_macro_attribute]
pub fn handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = syn::parse_macro_input!(item as syn::ItemFn);

    if !cfg!(feature = "trace") {
        return quote::quote!(#input_fn).into();
    }

    let fn_name = input_fn.sig.ident.to_string();
    let original_block = &input_fn.block;

    let new_block = quote::quote! {
        {
            println!("[vpratt trace] ➡ Executing: {}", #fn_name);
            #original_block
        }
    };

    input_fn.block = Box::new(syn::parse_quote!(#new_block));
    quote::quote!(#input_fn).into()
}

#[proc_macro_attribute]
pub fn vpratt_token(args: TokenStream, input: TokenStream) -> TokenStream {
    token::expand(args.into(), input.into())
        .unwrap_or_else(|e| e.to_compile_error().into())
        .into()
}
