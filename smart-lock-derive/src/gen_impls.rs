use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let struct_name = &parsed.name;
    let lock_name = format_ident!("{}Lock", struct_name);

    let (_, ty_generics, _) = parsed.generics.split_for_impl();
    let impl_params = parsed.impl_generic_params();
    let has_generics = parsed.has_generics();

    let lock_name_str = lock_name.to_string();

    // --- Debug impl ---
    let debug_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let name_str = name.to_string();
            quote! {
                match self.#name.try_read() {
                    Some(guard) => { s.field(#name_str, &*guard); }
                    None => { s.field(#name_str, &format_args!("<locked>")); }
                }
            }
        })
        .collect();

    let debug_bounds: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            quote! { #ty: std::fmt::Debug }
        })
        .collect();

    let existing_where_predicates = parsed.generics.where_clause.as_ref().map(|w| {
        let predicates = &w.predicates;
        quote! { #predicates, }
    }).unwrap_or_default();

    let debug_impl = if has_generics {
        quote! {
            impl<#(#impl_params),*> std::fmt::Debug for #lock_name #ty_generics
            where
                #existing_where_predicates
                #(#debug_bounds),*
            {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut s = f.debug_struct(#lock_name_str);
                    #(#debug_fields)*
                    s.finish()
                }
            }
        }
    } else {
        quote! {
            impl std::fmt::Debug for #lock_name
            where
                #(#debug_bounds),*
            {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut s = f.debug_struct(#lock_name_str);
                    #(#debug_fields)*
                    s.finish()
                }
            }
        }
    };

    // --- Default impl ---
    let default_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            quote! {
                #name: smart_lock::RwLock::new(Default::default()),
            }
        })
        .collect();

    let default_bounds: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            quote! { #ty: Default }
        })
        .collect();

    let default_impl = if has_generics {
        quote! {
            impl<#(#impl_params),*> Default for #lock_name #ty_generics
            where
                #existing_where_predicates
                #(#default_bounds),*
            {
                fn default() -> Self {
                    Self {
                        #(#default_fields)*
                    }
                }
            }
        }
    } else {
        quote! {
            impl Default for #lock_name
            where
                #(#default_bounds),*
            {
                fn default() -> Self {
                    Self {
                        #(#default_fields)*
                    }
                }
            }
        }
    };

    quote! {
        #debug_impl
        #default_impl
    }
}
