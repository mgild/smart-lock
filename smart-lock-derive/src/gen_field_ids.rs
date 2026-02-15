use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let vis = &parsed.vis;
    let struct_name = &parsed.name;
    let field_id_name = format_ident!("{}FieldId", struct_name);

    let constants: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let field_name = &field.name;
            quote! {
                #[allow(non_upper_case_globals)]
                #vis const #field_name: #field_id_name = #field_id_name(#i);
            }
        })
        .collect();

    // For generic structs, we put the constants on a non-generic impl block
    // by using the FieldId type itself (which is not generic)
    let (impl_generics, ty_generics, where_clause) = parsed.generics.split_for_impl();

    quote! {
        #vis struct #field_id_name(pub usize);

        impl smart_lock::FieldId for #field_id_name {
            fn index(&self) -> usize {
                self.0
            }
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            #(#constants)*
        }
    }
}
