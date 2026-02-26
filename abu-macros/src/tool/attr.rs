use syn::{parse::{Parse, ParseStream}, punctuated::Punctuated, Ident, LitStr, Result, Token};

pub struct ToolAttr {
    pub struct_name: Ident,
    pub name: Option<LitStr>,
    pub description: LitStr,
}

impl Parse for ToolAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut struct_name: Option<Ident> = None;
        let mut name: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        let metas = Punctuated::<syn::Meta, Token![,]>::parse_terminated(input)?;
    
        for meta in metas {
            match meta {
                syn::Meta::NameValue(nv) => {
                    let ident = nv.path.get_ident()
                        .ok_or_else(|| syn::Error::new_spanned(&nv.path, "expected ident"))?
                        .to_string();

                    match ident.as_str() {
                        "struct_name" => {
                            if let syn::Expr::Path(expr_path) = nv.value {
                                if let Some(id) = expr_path.path.get_ident() {
                                    struct_name = Some(id.clone());
                                } else {
                                    return Err(syn::Error::new_spanned(expr_path, "invalid struct_name"));
                                }
                            } else {
                                return Err(syn::Error::new_spanned(nv.value, "expected ident"));
                            }
                        }
                        "name" => {
                            if let syn::Expr::Lit(expr_lit) = nv.value {
                                if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                    name = Some(lit_str);
                                } else {
                                    return Err(syn::Error::new_spanned(expr_lit, "expected string"));
                                }
                            }
                        }
                        "description" => {
                            if let syn::Expr::Lit(expr_lit) = nv.value {
                                if let syn::Lit::Str(lit_str) = expr_lit.lit {
                                    description = Some(lit_str);
                                } else {
                                    return Err(syn::Error::new_spanned(expr_lit, "expected string"));
                                }
                            }
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                nv.path,
                                "unknown attribute key"
                            ));
                        }
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(meta, "unsupported meta"));
                }
            }
        }

        Ok(ToolAttr {
            struct_name: struct_name.ok_or_else(|| {
                syn::Error::new(input.span(), "struct_name is required")
            })?,
            name,
            description: description.ok_or_else(|| {
                syn::Error::new(input.span(), "description is required")
            })?,
        })
    }
}
