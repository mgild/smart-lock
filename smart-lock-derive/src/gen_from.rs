use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let struct_name = &parsed.name;
    let lock_name = format_ident!("{}Lock", struct_name);

    let (impl_generics, ty_generics, where_clause) = parsed.generics.split_for_impl();

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
        impl #impl_generics From<#struct_name #ty_generics> for #lock_name #ty_generics #where_clause {
            fn from(value: #struct_name #ty_generics) -> Self {
                Self {
                    #(#field_inits)*
                }
            }
        }
    }
}
