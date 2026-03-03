mod tool;
mod fastmcp;
mod utils;

#[proc_macro_attribute]
pub fn tool(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    tool::tool_impl(attr, item)
}

#[proc_macro_attribute]
pub fn mcp_tool(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    fastmcp::tool_impl(attr, item)
}
