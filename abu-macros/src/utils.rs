use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

pub fn get_abu_path() -> proc_macro2::TokenStream {
    match crate_name("abu-core") {
        Ok(FoundCrate::Itself) => {
            quote! { crate }
        }
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote! { #ident }
        }
        Err(_) => {
            quote! { abu_core }
        }
    }
}