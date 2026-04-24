use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, Ident, ItemFn, PathArguments, ReturnType, Type
};
use proc_macro2;
use quote::quote;

pub struct Param {
    pub name: Ident,
    pub typ: Type,
    pub is_reference: bool,
    pub description: Option<String>,
    pub default: Option<String>,
}

pub fn parse_params(input_fn: &mut ItemFn) -> (Vec<Param>, bool) {
    let mut is_associated = false;
    let inputs = &mut input_fn.sig.inputs;

    let mut params = vec![];
    for arg in inputs.iter_mut() {
        if let FnArg::Typed(pat_type) = arg {
            let param_name = if let syn::Pat::Ident(ident) = &*pat_type.pat {
                &ident.ident
            } else {
                panic!("Expected ident")
            };

            let mut param_type = (*pat_type.ty).clone();
            let mut is_reference = false;

            if let Type::Reference(type_ref) = &param_type {
                is_reference = true;
                let elem = type_ref.elem.as_ref();
                
                if let Type::Path(tp) = elem {
                    if tp.path.is_ident("str") {
                        param_type = syn::parse_quote!(String);
                    } else {
                        param_type = elem.clone();
                    }
                } else if let Type::Slice(slice) = elem {
                    let inner = &slice.elem;
                    param_type = syn::parse_quote!(Vec<#inner>);
                } else {
                    param_type = elem.clone();
                }
            }
            
            let mut next_attrs = Vec::new();
            let mut description = None;
            let mut default = None;
            for attr in &pat_type.attrs {
                if attr.path().is_ident("arg") {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("description") {
                            let value: syn::LitStr = meta.value()?.parse()?;
                            description = Some(value.value());
                            Ok(())
                        } else if meta.path.is_ident("default") {
                            let value: syn::LitStr = meta.value()?.parse()?;
                            default = Some(value.value());
                            Ok(())
                        } else {
                            Err(meta.error("unknown key in arg"))
                        }
                    }).expect("parse meta");
                } else {
                    next_attrs.push(attr.clone());
                }
            }
            pat_type.attrs = next_attrs;

            params.push(Param {
                name: param_name.clone(),
                typ: param_type, 
                is_reference,
                default,
                description,
            });
        } else {
            is_associated = true;
        }  
    }

    (params, is_associated)
}

pub fn generate_args_transform_code(params_info: &[Param]) -> Vec<proc_macro2::TokenStream> {
    let abu = crate::utils::get_abu_path();
    let mut args_trans_code = Vec::new();
    
    for param in params_info {
        let arg_name = &param.name;
        let arg_name_str = arg_name.to_string();
        let typ = &param.typ; 

        let code = match &param.default {
            None => quote! {
                let #arg_name = {
                    let val = args.get(#arg_name_str).cloned().ok_or_else(|| #abu::ToolError::ArgNotFound(#arg_name_str.to_string()) )?;                
                    <#typ as #abu::ToolArgument>::from_value(val).map_err(|e| #abu::ToolError::ArgParse(stringify!(#typ)))?
                };
            },
            Some(default) => {
                let default_expr: syn::Expr = syn::parse_str(default).expect("Invalid default value expression");
                quote! {
                    let #arg_name = match args.get(#arg_name_str).cloned() {
                        None => #default_expr,
                        Some(val) => <#typ as #abu::ToolArgument>::from_value(val).map_err(|e| #abu::ToolError::ArgParse(stringify!(#typ)))?,
                    };
                }
            }
        };

        args_trans_code.push(code);
    }
    args_trans_code
}

pub fn generate_parameters(params: &[Param]) -> proc_macro2::TokenStream {
    let properties = params.iter()
        .map(generate_parameter);

    quote! {
        #(#properties),*
    }
}

pub fn generate_parameter(param: &Param) -> proc_macro2::TokenStream {
    let abu = crate::utils::get_abu_path();
    let name = param.name.to_string();
    let typ = &param.typ;

    let mut code = quote! {
        #abu::ToolParameter {
            name: #name.to_string(),
            required: false,
            description: None,
            kind: <#typ as #abu::ToolArgument>::parameter_kind(),
        }
    };

    if let Some(desc) = &param.description {
        code = quote! { #code.description(#desc) }
    }

    if let Some(_) = &param.default {
        code = quote! { #code.required(true) }
    }

    code
}

pub fn generate_return_code(input_fn: &ItemFn, params_info: &[Param], struct_name: &Ident, is_associated: bool) -> proc_macro2::TokenStream {
    let abu = crate::utils::get_abu_path();
    let fn_name = &input_fn.sig.ident;
    let async_mark = if input_fn.sig.asyncness.is_none() { quote! { } } else { quote! { .await } };
    
    let mut args = Vec::new();
    for param in params_info.iter() {
        let arg_name = &param.name;
        
        if param.is_reference {
            args.push(quote! { &#arg_name });
        } else {
            args.push(quote! { #arg_name });
        }
    }

    let fn_invoke = if is_associated {
        quote! { self.#fn_name(#(#args),*)#async_mark } 
    } else { 
        quote! { #struct_name::#fn_name(#(#args),*)#async_mark }
    };

    match &input_fn.sig.output {
        // 情况 1: 没有任何返回值定义 (fn foo())
        ReturnType::Default => quote! {
            #fn_invoke;
            Ok(#abu::ToolCallResult::success("no output"))
        },
        ReturnType::Type(_, ty) => {
            // 情况 2: 显式定义返回 () (fn foo() -> ())
            let is_explicit_unit = matches!(ty.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty());
    
            if is_explicit_unit {
                quote! {
                    #fn_invoke;
                    Ok(#abu::ToolCallResult::success("no output"))
                }
            } else {
                // 情况 3: 有具体返回类型 (Result 或 其他类型)
                if let Type::Path(type_path) = ty.as_ref() {
                    if let Some(segment) = type_path.path.segments.last() {
                        // 检查是否是 Result
                        if segment.ident.to_string().contains("Result") {
                            if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = &segment.arguments {
                                if let Some(GenericArgument::Type(inner_ty)) = args.first() {
                                    // 检查 Result 的 Ok 类型是否为 ()
                                    if matches!(inner_ty, Type::Tuple(tuple) if tuple.elems.is_empty()) {
                                        // Result<()>
                                        quote! {
                                            let result = #fn_invoke;
                                            match result {
                                                Ok(()) => Ok(#abu::ToolCallResult::success("no output")),
                                                Err(err) => Ok(#abu::ToolCallResult::error(err.to_string())),
                                            }
                                        }
                                    } else {
                                        // Result<T>
                                        quote! {
                                            let result = #fn_invoke;
                                            match result {
                                                Ok(value) => Ok(#abu::ToolCallResult::success(format!("{}", value))),
                                                Err(err) => Ok(#abu::ToolCallResult::error(err.to_string())),
                                            }
                                        }
                                    }
                                } else {
                                    // 无法解析泛型参数，回退到普通处理
                                    quote! { Ok(#abu::ToolCallResult::success(format!("{}", #fn_invoke))) } 
                                }
                            } else {
                                // Result 但没有泛型参数?
                                quote! { Ok(#abu::ToolCallResult::success(format!("{}", #fn_invoke))) }
                            }
                        } else {
                            // 普通返回值 (如 String, i32 等)
                            quote! {
                                Ok(#abu::ToolCallResult::success(format!("{}", #fn_invoke)))
                            }
                        }
                    } else {
                        panic!("No path segment found!")
                    }
                } else {
                    unimplemented!("No support for non-path return types")
                }
            }
        }
    }
}

// fn param_type_to_string(tp: ParamType) -> &'static str {
//     match tp {
//         ParamType::I64 => "i64",
//         ParamType::USize => "i64",
//         ParamType::Str => "string",
//         ParamType::String => "string",
//     }
// }