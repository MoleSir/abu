use syn::{
    bracketed, 
    parse::{Parse, ParseStream}, 
    Ident,
    LitStr, Result, 
    Token,
};
use proc_macro2::TokenStream as TokenStream2;

pub struct ToolAttr {
    pub struct_name: Ident,
    pub name: LitStr,
    pub description: LitStr,
    pub params: TokenStream2,
}

impl Parse for ToolAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let _struct_name = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let struct_name_value = input.parse::<Ident>()?;
        input.parse::<Token![,]>()?;

        let _name = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let name_value = input.parse::<LitStr>()?;
        input.parse::<Token![,]>()?;

        let _description = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let description_value = input.parse::<LitStr>()?;
        input.parse::<Token![,]>()?;

        let _arg_name = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;

        let content;
        bracketed!(content in input);
        let params: TokenStream2 = content.parse::<TokenStream2>()?;

        Ok(ToolAttr {
            struct_name: struct_name_value,
            name: name_value,
            description: description_value,
            params,
        })
    }
}
