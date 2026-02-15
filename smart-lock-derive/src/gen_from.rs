use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let struct_name = &parsed.name;
    let lock_name = format_ident!("{}Lock", struct_name);

    let impl_prefix = parsed.impl_prefix();
    let ty_generics = parsed.ty_generics();
    let where_clause = parsed.where_clause();

    let field_inits: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            quote! {
                #name: smart_lock::RwLock::new(value.#name),
            }
        })
        .collect();

    quote! {
        impl<#impl_prefix> From<#struct_name #ty_generics> for #lock_name #ty_generics #where_clause {
            fn from(value: #struct_name #ty_generics) -> Self {
                Self {
                    #(#field_inits)*
                }
            }
        }
    }
}
