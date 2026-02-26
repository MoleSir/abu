use syn::{
    AngleBracketedGenericArguments, FnArg, GenericArgument, Ident, ItemFn, PathArguments, ReturnType, Type
};
use proc_macro2;
use quote::quote;

pub enum ParamType {
    I64,
    USize,
    Str,
    String,
}

pub struct Param {
    pub name: Ident,
    pub typ: ParamType,
}

pub fn parse_params(input_fn: &ItemFn) -> Vec<Param> {
    let inputs = &input_fn.sig.inputs;

    inputs.iter().map(|arg| -> Param {
        if let FnArg::Typed(pat_type) = arg {
            let param_name = if let syn::Pat::Ident(ident) = &*pat_type.pat {
                &ident.ident
            } else {
                panic!("")
            };

            let param_type = &*pat_type.ty;

            let param_type_enum = match param_type {
                Type::Path(type_path) if type_path.path.is_ident("i64") => ParamType::I64,
                Type::Path(type_path) if type_path.path.is_ident("String") => ParamType::String,
                Type::Path(type_path) if type_path.path.is_ident("usize") => ParamType::USize,
                Type::Reference(type_ref) if matches!(type_ref.elem.as_ref(), Type::Path(tp) if tp.path.is_ident("str")) => ParamType::Str,
                _ => panic!("")
            };

            Param {
                name: param_name.clone(),
                typ: param_type_enum,
            }  
        } else {
            panic!("")
        }  
    }).collect()
}

pub fn generate_args_transform_code(params_info: &[Param]) -> Vec<proc_macro2::TokenStream> {
    let abu = crate::utils::get_abu_path();
    let mut args_trans_code = Vec::new();
    for param in params_info {
        let arg_name = &param.name;
        let arg_name_str = arg_name.to_string();
        let trans_code = match param.typ {
            ParamType::I64 => quote! { as_i64().ok_or_else(|| #abu::tool::ToolError::ArgParse("i64"))? },
            ParamType::USize => quote ! { as_i64().ok_or_else(|| #abu::tool::ToolError::ArgParse("i64"))? },
            ParamType::Str => quote! { as_str().ok_or_else(|| #abu::tool::ToolError::ArgParse("string"))? },
            ParamType::String => quote! { as_str().ok_or_else(|| #abu::tool::ToolError::ArgParse("string"))? },
        };
        args_trans_code.push(quote! {
            let #arg_name = args
                .get(#arg_name_str)
                .ok_or_else(|| #abu::tool::ToolError::ArgNotFound(#arg_name_str.to_string()))?
                .#trans_code;
        });
    }
    args_trans_code
}

pub fn generate_required_list_code(params_info: &[Param]) -> Vec<proc_macro2::TokenStream> {
    let mut required_list_code: Vec<proc_macro2::TokenStream> = Vec::new();
    for (i, param) in params_info.iter().enumerate() {
        let arg_name = &param.name;
        let arg_name_str = arg_name.to_string();
        
        if i != 0 {
            required_list_code.push(quote! { , });
        }
        required_list_code.push(quote! {
            #arg_name_str
        });
    }
    required_list_code
}

pub fn generate_return_code(input_fn: &ItemFn, params_info: &[Param], struct_name: &Ident) -> proc_macro2::TokenStream {
    let fn_name = &input_fn.sig.ident;
    let async_mark = if input_fn.sig.asyncness.is_none() { quote! { } } else { quote! { .await } };
    let mut args = Vec::new();
    for (i, param) in params_info.iter().enumerate() {
        let arg_name = &param.name;
        if i != 0 {
            args.push(quote! { , });
        }
        args.push(quote! { #arg_name });                
    }

    let fn_invoke = quote! { #struct_name::#fn_name(#(#args)*)#async_mark };
    match &input_fn.sig.output {
        // 情况 1: 没有任何返回值定义 (fn foo())
        ReturnType::Default => quote! {
            #fn_invoke;
            Ok(format!("no output"))
        },
        ReturnType::Type(_, ty) => {
            // 情况 2: 显式定义返回 () (fn foo() -> ())
            let is_explicit_unit = matches!(ty.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty());
    
            if is_explicit_unit {
                quote! {
                    #fn_invoke;
                    Ok(format!("no output"))
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
                                                Ok(()) => Ok(format!("Execute success!")),
                                                Err(err) => Ok(format!("Execute failed for {}", err)),
                                            }
                                        }
                                    } else {
                                        // Result<T>
                                        quote! {
                                            let result = #fn_invoke;
                                            match result {
                                                Ok(value) => Ok(format!("Execute success and return {}", value)),
                                                Err(err) => Ok(format!("Execute failed for {}", err)),
                                            }
                                        }
                                    }
                                } else {
                                    // 无法解析泛型参数，回退到普通处理
                                    quote! { Ok(format!("{}", #fn_invoke)) } 
                                }
                            } else {
                                // Result 但没有泛型参数?
                                 quote! { Ok(format!("{}", #fn_invoke)) }
                            }
                        } else {
                            // 普通返回值 (如 String, i32 等)
                            quote! {
                                Ok(format!("{}", #fn_invoke))
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