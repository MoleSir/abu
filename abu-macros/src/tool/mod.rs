mod attr;
mod func;

use attr::ToolAttr;
use func::*;

use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, ItemFn, Lit, LitStr, Meta
};

pub fn tool_impl(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let abu = crate::utils::get_abu_path();

    let mut input_fn = parse_macro_input!(item as ItemFn);
    let tool_attr = parse_macro_input!(attr as ToolAttr);

    // Parse attr
    let struct_name = tool_attr.struct_name;
    let name = tool_attr.name.map(|m| m.value()).unwrap_or_else(|| input_fn.sig.ident.to_string());
    let description = tool_attr.description.value();

    // Parse function
    let (params_info, is_associated) = parse_params(&mut input_fn);
    let args_trans_code = generate_args_transform_code(&params_info);
    let return_code = generate_return_code(&input_fn, &params_info, &struct_name, is_associated);
    let parameters = generate_parameters(&params_info);

    let code = if is_associated {
        quote! {}
    } else {
        quote! {
            pub struct #struct_name;

            impl #struct_name {
                pub fn new() -> Self {
                    Self
                }
            }
        }
    };

    let code = quote! {
        #code 

        impl #struct_name {
           #input_fn
        }

        #[async_trait::async_trait]
        impl #abu::Tool for #struct_name {
            fn name(&self) -> &'static str {
                #name
            }
        
            fn description(&self) -> &'static str {
                #description
            }

            fn parameters(&self) -> Vec<#abu::ToolParameter> {
                vec![ #parameters ]
            }

            async fn execute(&self, args: #abu::_serde_json::Value) -> std::result::Result<#abu::ToolCallResult, #abu::ToolError> {
                #(#args_trans_code)*
                #return_code
            }
        }
        
    };    

    code.into()
}

pub fn tool_argument_impl(_args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);
    let abu = crate::utils::get_abu_path();

    // 1. 解析 rename_all 逻辑
    let mut rename_rule = None;
    
    // 检查属性 #[tool_argument(rename_all = "...")]
    // 或者检查结构体上的 #[tool(rename_all = "...")]
    input.attrs.retain(|attr| {
        if attr.path().is_ident("tool") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    let value: LitStr = meta.value()?.parse()?;
                    rename_rule = Some(value.value());
                }
                Ok(())
            });
            // 我们可以选择保留或删除这个属性
            return true;
        }
        true
    });

    let serde_rename = if let Some(ref rule) = rename_rule {
        quote! { #[serde(rename_all = #rule)] }
    } else {
        quote! {}
    };

    // 2. 重新构造输出
    // 自动添加 Deserialize 和 ToolArgument 派生
    // 自动添加 serde 属性映射
    let expanded = quote! {
        #[derive(#abu::_serde::Deserialize, #abu::ToolArgument)]
        #serde_rename
        #input
    };

    expanded.into()
}

pub fn derive_tool_argument_impl(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let abu = crate::utils::get_abu_path();

    // 1. #[serde(rename_all = "...")] / #[tool(rename_all = "...")]
    let mut rename_all = None;
    for attr in &input.attrs {
        if attr.path().is_ident("serde") || attr.path().is_ident("tool") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename_all") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    rename_all = Some(value.value());
                }
                Ok(())
            });
        }
    }

    let expanded = match input.data {
        Data::Enum(data_enum) => {
            let mut variant_names = Vec::new();
            for variant in data_enum.variants {
                let mut name = variant.ident.to_string();

                // 应用 rename_all 规则
                if let Some(rule) = &rename_all {
                    match rule.as_str() {
                        "lowercase" => name = name.to_lowercase(),
                        "uppercase" => name = name.to_uppercase(),
                        "snake_case" => name = to_snake_case(&name),
                        _ => {} 
                    }
                }
                variant_names.push(name);
            }

            quote::quote! {
                impl #abu::ToolArgument for #ident {
                    fn parameter_kind() -> #abu::ToolParameterKind {
                        #abu::ToolParameterKind::String(Some(vec![
                            #( #variant_names.to_string() ),*
                        ]))
                    }
                }
            }
        }

        Data::Struct(data_struct) => {
            let mut fields_code = Vec::new();
            if let syn::Fields::Named(fields) = data_struct.fields {
                for field in fields.named {
                    let field_name = field.ident.unwrap().to_string();
                    let field_type = field.ty;

                    let description = get_description(&field.attrs);
                    let description_quote = match description {
                        Some(d) => quote! { Some(#d.to_string()) },
                        None => quote! { None },
                    };
                    
                    fields_code.push(quote::quote! {
                        #abu::ToolParameter {
                            name: #field_name.to_string(),
                            required: true,
                            description: #description_quote, 
                            kind: <#field_type as #abu::ToolArgument>::parameter_kind(),
                        }
                    });
                }
            }

            quote::quote! {
                impl #abu::ToolArgument for #ident {
                    fn parameter_kind() -> #abu::ToolParameterKind {
                        #abu::ToolParameterKind::Object(vec![
                            #( #fields_code ),*
                        ])
                    }
                }
            }
        }
        _ => panic!("ToolArgument can only be derived for enums and structs"),
    };

    expanded.into()
}

// 辅助函数：从属性列表中提取 doc 注释或 tool(description)
fn get_description(attrs: &[Attribute]) -> Option<String> {
    let mut tool_desc = None;
    let mut doc_lines = Vec::new();

    for attr in attrs {
        // 1. 尝试解析 #[tool(description = "...")]
        if attr.path().is_ident("tool") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("description") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    tool_desc = Some(value.value());
                }
                Ok(())
            });
        }
        
        // 2. 尝试解析 /// 文档注释
        // 文档注释在 Rust 中被存储为 #[doc = "..."]
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) = &nv.value {
                    doc_lines.push(s.value().trim().to_string());
                }
            }
        }
    }

    // 优先级：手动指定的 description > 文档注释
    if tool_desc.is_some() {
        tool_desc
    } else if !doc_lines.is_empty() {
        Some(doc_lines.join(" "))
    } else {
        None
    }
}

fn to_snake_case(s: &str) -> String {
    let mut snake = String::new();
    for (i, ch) in s.char_indices() {
        if i > 0 && ch.is_uppercase() {
            snake.push('_');
        }
        snake.push(ch.to_ascii_lowercase());
    }
    snake
}