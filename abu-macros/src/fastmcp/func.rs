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
                panic!("???")
            };

            let param_type = &*pat_type.ty;

            let param_type_enum = match param_type {
                Type::Path(type_path) if type_path.path.is_ident("i64") => ParamType::I64,
                Type::Path(type_path) if type_path.path.is_ident("String") => ParamType::String,
                Type::Path(type_path) if type_path.path.is_ident("usize") => ParamType::USize,
                Type::Reference(type_ref) if matches!(type_ref.elem.as_ref(), Type::Path(tp) if tp.path.is_ident("str")) => ParamType::Str,
                _ => panic!("Unsupport param type")
            };

            Param {
                name: param_name.clone(),
                typ: param_type_enum,
            }  
        } else {
            panic!("Unsupport param self type")
        }  
    }).collect()
}

pub fn generate_args_transform_code(params_info: &[Param]) -> Vec<proc_macro2::TokenStream> {
    let mut args_trans_code = Vec::new();
    for param in params_info {
        let arg_name = &param.name;
        let arg_name_str = arg_name.to_string();
        let trans_code = match param.typ {
            ParamType::I64 => quote! { as_i64().context("Expect i64")? },
            ParamType::USize => quote ! { as_i64().context("Expect i64")? as usize },
            ParamType::Str => quote! { as_str().context("Expect &str")? },
            ParamType::String => quote! { as_str().context("Expect String")?.to_string() },
        };
        args_trans_code.push(quote! {
            let #arg_name = args.get(#arg_name_str).context("Bad args")?.#trans_code;
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
        ReturnType::Default => quote! { 
            #fn_invoke;
            Ok(None) 
        },
        ReturnType::Type(_, ty) => {
            if let Type::Path(type_path) = ty.as_ref() {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Result" {
                        // Result<?>
                        if let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) = &segment.arguments {
                            if let Some(GenericArgument::Type(inner_ty)) = args.first() {
                                // 判断 inner_ty 是否为 ()
                                if matches!(inner_ty, Type::Tuple(tuple) if tuple.elems.is_empty()) {
                                    // No return value!
                                    quote! {
                                        let result = #fn_invoke;
                                        match result {
                                            Ok(()) => Ok(Some(format!("Execute success!"))),
                                            Err(err) => Ok(Some(format!("Execute failed for {}", err))),
                                        }
                                    }
                                } else {
                                    quote! {
                                        let result = #fn_invoke;
                                        match result {
                                            Ok(value) => Ok(Some(format!("Execute success and return {}", value))),
                                            Err(err) => Ok(Some(format!("Execute failed for {}", err))),
                                        }
                                    }
                                }
                            } else {
                                panic!()
                            }
                        }else {
                            panic!()
                        }
                    } else {
                        // Return value
                        quote! {
                            Ok(Some(format!("{}", #fn_invoke)))
                        }
                    }
                } else {
                    panic!("No input code!")
                }
            } else {
                unimplemented!("No support un path return type")
            }
        }
    }
}