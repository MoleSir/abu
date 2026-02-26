mod tool;
mod utils;

#[proc_macro_attribute]
pub fn tool(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    tool::tool_impl(attr, item)
}