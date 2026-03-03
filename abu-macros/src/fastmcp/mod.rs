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
    // Parse attr
    let tool_attr = parse_macro_input!(attr as ToolAttr);
    let struct_name = tool_attr.struct_name;
    let name = tool_attr.name.value();
    let description = tool_attr.description.value();
    let params = tool_attr.params;

    // Parse function
    let input_fn = parse_macro_input!(item as ItemFn);
    let params_info = parse_params(&input_fn);
    let args_trans_code = generate_args_transform_code(&params_info);
    let required_list_code = generate_required_list_code(&params_info);
    let return_code = generate_return_code(&input_fn, &params_info, &struct_name);

    let code = quote! {
        pub struct #struct_name;

        #[async_trait::async_trait]
        impl Tool for #struct_name {
            fn to_mcptool(&self) -> McpTool {
                McpTool {
                    name: #name.to_string(),
                    description: Some(#description.to_string()),
                    input_schema: McpToolInputSchema {
                        r#type: "object".to_string(),
                        required: Some(serde_json::json!([#(#required_list_code)*])),
                        properties: Some(
                            serde_json::json!({
                                #params
                            })
                        ),
                    }
                }
            }

            fn name(&self) -> String {
                #name.to_string()
            }
    
            async fn execute(&self, args: serde_json::Value) -> anyhow::Result<Option<String>> {
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
