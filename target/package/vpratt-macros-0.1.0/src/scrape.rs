use syn::spanned::Spanned;
use syn::{Expr, ImplItem, ItemImpl};
use crate::table::{Assoc, Entry, TableDef};

/// Scrapes the `ItemImpl` to find the `const TABLE` and extracts its entries.
pub fn scrape_table(impl_block: &mut ItemImpl) -> syn::Result<TableDef> {
    let mut table_expr = None;

    for item in &mut impl_block.items {
        if let ImplItem::Const(impl_const) = item {
            if impl_const.ident == "TABLE" {
                table_expr = Some(impl_const.expr.clone());
                break;
            }
        }
    }

    let table_expr = table_expr.ok_or_else(|| {
        syn::Error::new_spanned(
            impl_block,
            "Missing `const TABLE: vpratt::Table<Self> = ...;` in the impl block,",
        )
    })?;

    // 2. Unwind the method chain into the IR
    let mut entries = Vec::new();
    scrape_chain(&table_expr, &mut entries)?;

    Ok(TableDef { entries })
}

fn scrape_chain(expr: &Expr, entries: &mut Vec<Entry>) -> syn::Result<()> {
    match expr {
        Expr::MethodCall(call) => {
            // 1. Recurse down to the receiver first to maintain top-to-bottom order
            scrape_chain(&call.receiver, entries)?;

            let method = call.method.to_string();
            let args = &call.args;
            let span = call.span();

            // 2. Map the builder methods to IR
            match method.as_str() {
                "terminal" => {
                    require_args(args, 2, "terminal")?;
                    entries.push(Entry::Terminal {
                        token: args[0].clone(),
                        handler: args[1].clone(),
                        span,
                    });
                }
                "group" => {
                    require_args(args, 3, "group")?;
                    entries.push(Entry::Group {
                        open: args[0].clone(),
                        close: args[1].clone(),
                        handler: args[2].clone(),
                        span,
                    });
                }
                "prefix" => {
                    require_args(args, 3, "prefix")?;
                    entries.push(Entry::Prefix {
                        bp: args[0].clone(),
                        token: args[1].clone(),
                        handler: args[2].clone(),
                        span,
                    });
                }
                "infix" => {
                    require_args(args, 4, "infix")?;
                    entries.push(Entry::Infix {
                        bp: args[0].clone(),
                        assoc: parse_assoc(&args[1])?,
                        token: args[2].clone(),
                        handler: args[3].clone(),
                        span,
                    });
                }
                "postfix" => {
                    require_args(args, 3, "postfix")?;
                    entries.push(Entry::Postfix {
                        bp: args[0].clone(),
                        token: args[1].clone(),
                        handler: args[2].clone(),
                        span,
                    });
                }
                "juxt" => {
                    entries.push(Entry::Juxt {
                        bp: args[0].clone(),
                        open: args[1].clone(),
                        close: args[2].clone(),
                        handler: args[3].clone(),
                        span: method.span(),
                    });
                }

                "implied" => {
                    entries.push(Entry::Implied {
                        bp: args[0].clone(),
                        assoc: parse_assoc(&args[1])?,
                        token: args[2].clone(),
                        handler: args[3].clone(),
                        span: method.span(),
                    });
                }
                _ => {
                    return Err(syn::Error::new(
                        call.method.span(),
                        format!("Unknown vpratt table layout: `{}`", method),
                    ));
                }
            }
            Ok(())
        }

        Expr::Call(_) | Expr::Path(_) => {
            Ok(())
        }

        _ => Err(syn::Error::new_spanned(
            expr,
            "Expected a method call chain starting with `vpratt::Table::new()`",
        )),
    }
}

/// Enforces the exact number of arguments for a given layout method
fn require_args(
    args: &syn::punctuated::Punctuated<Expr, syn::token::Comma>,
    expected: usize,
    name: &str,
) -> syn::Result<()> {
    if args.len() != expected {
        Err(syn::Error::new_spanned(
            args,
            format!("`.{name}()` expects exactly {expected} arguments, but got {}", args.len()),
        ))
    } else {
        Ok(())
    }
}

/// Safely extracts the `Associativity::Left` or `Right` identifier
fn parse_assoc(expr: &Expr) -> syn::Result<Assoc> {
    if let Expr::Path(path) = expr {
        if let Some(ident) = path.path.segments.last().map(|s| s.ident.to_string()) {
            match ident.as_str() {
                "Left" => return Ok(Assoc::Left),
                "Right" => return Ok(Assoc::Right),
                _ => {} // Fall through to error
            }
        }
    }

    Err(syn::Error::new_spanned(
        expr,
        "Expected `Left` or `Right` for associativity. \
        Ensure you imported `vpratt::Associativity::{Left, Right}`."
    ))
}