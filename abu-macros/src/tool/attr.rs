use syn::{parse::{Parse, ParseStream}, punctuated::Punctuated, Expr, Ident, Result, Token};

pub struct ToolAttr {
    pub struct_name: Ident,
    pub name: Option<Expr>,
    pub description: Expr,
    pub category: Option<String>,
    pub generics: Option<syn::Generics>, 
}

impl Parse for ToolAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut struct_name: Option<Ident> = None;
        let mut name: Option<Expr> = None;
        let mut description: Option<Expr> = None;
        let mut category: Option<String> = None;
        let mut generics: Option<syn::Generics> = None;

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
                        "name" => name = Some(nv.value),
                        "description" => description = Some(nv.value),
                        "category" => {
                            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = nv.value {
                                category = Some(s.value());
                            } else {
                                return Err(syn::Error::new_spanned(nv.value, "category must be a string literal"));
                            }
                        }
                        "generics" => {
                            // 解析泛型字符串，例如 "P: ChatProvide, M: Memory"
                            if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = nv.value {
                                // 我们在前后加上 <> 以便 syn 能够解析为 Generics 对象
                                let gen_str = format!("<{}>", s.value());
                                let code = syn::parse_str::<syn::Generics>(&gen_str)?;
                                generics = Some(code);
                            } else {
                                return Err(syn::Error::new_spanned(nv.value, "generics must be a string literal"));
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
            category,
            generics,
        })
    }
}
