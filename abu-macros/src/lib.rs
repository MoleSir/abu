mod tool;
mod utils;

#[proc_macro_attribute]
pub fn tool(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    tool::tool_impl(attr, item)
}

#[proc_macro_derive(ToolArgument, attributes(tool))]
pub fn derive_tool_argument(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    tool::derive_tool_argument_impl(input)
}

#[proc_macro_attribute]
pub fn tool_argument(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    tool::tool_argument_impl(args, input)
}