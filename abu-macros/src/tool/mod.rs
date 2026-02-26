mod attr;
mod func;

use attr::ToolAttr;
use func::*;

use quote::quote;
use syn::{
    parse_macro_input, 
    ItemFn,
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
    let params_info = parse_params(&mut input_fn);
    let args_trans_code = generate_args_transform_code(&params_info);
    let required_list_code = generate_required_list_code(&params_info);
    let return_code = generate_return_code(&input_fn, &params_info, &struct_name);
    let properties = generate_params_properties(&params_info);

    let code = quote! {
        pub struct #struct_name;

        #[async_trait::async_trait]
        impl #abu::tool::Tool for #struct_name {
            fn name(&self) -> &'static str {
                #name
            }
        
            fn description(&self) -> &'static str {
                #description
            }
        
            fn parameters(&self) -> serde_json::Value {
                serde_json::json!(
                    {
                        "type": "object",
                        "properties": {
                            #properties
                        },
                        "required": [ #(#required_list_code)* ],
                    }
                )
            }
        
            async fn execute(&self, args: serde_json::Value) -> std::result::Result<String, #abu::tool::ToolError> {
                #(#args_trans_code)*
                #return_code
            }
        }

        impl #struct_name {
            pub fn new() -> Self {
                Self
            }

            #input_fn
        }
    };    

    code.into()
}
