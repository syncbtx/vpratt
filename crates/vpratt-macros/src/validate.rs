use std::collections::{HashMap, HashSet};
use quote::ToTokens;
use crate::table::{Assoc, Entry, TableDef};

pub fn validate(table: &TableDef) -> syn::Result<()> {
    // Tracks the associativity assigned to each Binding Power level
    let mut levels: HashMap<String, Assoc> = HashMap::new();

    // Tracks which tokens have been mapped to NUD (Prefix/Terminal/Group) 
    // and LED (Infix/Postfix/Juxt) phases. We use the stringified token as the key.
    let mut seen_nud: HashSet<String> = HashSet::new();
    let mut seen_led: HashSet<String> = HashSet::new();

    for entry in &table.entries {
        // 1. Validate Associativity Clashes
        if let Some((bp_expr, assoc)) = entry.associativity() {
            let bp_str = bp_expr.to_token_stream().to_string();

            if let Some(existing_assoc) = levels.get(&bp_str) {
                if existing_assoc != &assoc {
                    return Err(syn::Error::new_spanned(
                        bp_expr,
                        format!(
                            "Associativity conflict: Precedence level `{}` is already {:?}-associative. \
                            You cannot mix Left and Right associativity at the same binding power.",
                            bp_str, existing_assoc
                        ),
                    ));
                }
            } else {
                levels.insert(bp_str, assoc);
            }
        }

        // 2. Validate Duplicate Routing
        let (phase_set, phase_name, token_expr, span) = match entry {
            Entry::Terminal { token, span, .. } => (&mut seen_nud, "Terminal/Prefix/Group", token, span),
            Entry::Group { open, span, .. }     => (&mut seen_nud, "Terminal/Prefix/Group", open, span),
            Entry::Structural { token, span, .. } => (&mut seen_nud, "Terminal/Prefix/Group/Structural", token, span),
            Entry::Prefix { token, span, .. }   => (&mut seen_nud, "Terminal/Prefix/Group", token, span),
            Entry::Infix { token, span, .. }    => (&mut seen_led, "Infix/Postfix/Juxt", token, span),
            Entry::Postfix { token, span, .. }  => (&mut seen_led, "Infix/Postfix/Juxt", token, span),
            Entry::Juxt { open, span, .. }      => (&mut seen_led, "Infix/Postfix/Juxt", open, span),
            Entry::Implied { token, span, .. }  => (&mut seen_led, "Infix/Postfix/Juxt/Implied", token, span),
        };

        let token_str = token_expr.to_token_stream().to_string();

        if !phase_set.insert(token_str.clone()) {
            return Err(syn::Error::new(
                *span,
                format!(
                    "Duplicate routing: `{}` is already mapped as a {} handler. \
                    A token can only have one behavior per phase.",
                    token_str, phase_name
                ),
            ));
        }
    }

    Ok(())
}

// A quick helper on the Entry enum to extract associativity for the validator
impl Entry {
    fn associativity(&self) -> Option<(&syn::Expr, Assoc)> {
        match self {
            Entry::Infix { bp, assoc, .. } | Entry::Implied { bp, assoc, .. } => Some((bp, *assoc)),
            _ => None,
        }
    }
}