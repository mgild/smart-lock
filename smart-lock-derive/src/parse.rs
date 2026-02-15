use syn::{Attribute, Generics, Ident, Type, Visibility, ItemStruct, Fields};
use quote::quote;

pub struct ParsedField {
    pub name: Ident,
    pub ty: Type,
    #[allow(dead_code)]
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,
}

pub struct ParsedStruct {
    pub vis: Visibility,
    pub name: Ident,
    pub generics: Generics,
    pub fields: Vec<ParsedField>,
}

impl ParsedStruct {
    /// Returns bare generic params (no bounds) for use in type applications.
    /// e.g. for `<T: Clone, U: Send>` returns tokens for `T, U`
    pub fn bare_generic_params(&self) -> Vec<proc_macro2::TokenStream> {
        self.generics.params.iter().map(|p| match p {
            syn::GenericParam::Type(tp) => {
                let ident = &tp.ident;
                quote!(#ident)
            }
            syn::GenericParam::Lifetime(lp) => {
                let lt = &lp.lifetime;
                quote!(#lt)
            }
            syn::GenericParam::Const(cp) => {
                let ident = &cp.ident;
                quote!(#ident)
            }
        }).collect()
    }

    /// Returns full generic params (with bounds) for use in impl parameter lists.
    pub fn impl_generic_params(&self) -> Vec<proc_macro2::TokenStream> {
        self.generics.params.iter().map(|p| quote!(#p)).collect()
    }

    pub fn has_generics(&self) -> bool {
        !self.generics.params.is_empty()
    }
}

pub fn parse(attr: proc_macro2::TokenStream, item: &ItemStruct) -> syn::Result<ParsedStruct> {
    // No arguments accepted
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            &attr,
            "smart_lock takes no arguments. Usage: #[smart_lock]",
        ));
    }

    // Extract named fields only
    let named_fields = match &item.fields {
        Fields::Named(named) => &named.named,
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "smart_lock only supports structs with named fields",
            ));
        }
        Fields::Unit => {
            return Err(syn::Error::new_spanned(
                &item.ident,
                "smart_lock only supports structs with named fields",
            ));
        }
    };

    let fields: Vec<ParsedField> = named_fields
        .iter()
        .map(|f| ParsedField {
            name: f.ident.clone().unwrap(),
            ty: f.ty.clone(),
            vis: f.vis.clone(),
            attrs: f.attrs.clone(),
        })
        .collect();

    // Validate max 64 fields
    if fields.len() > 64 {
        return Err(syn::Error::new_spanned(
            &item.ident,
            format!(
                "smart_lock supports at most 64 fields, but `{}` has {}",
                item.ident,
                fields.len()
            ),
        ));
    }

    Ok(ParsedStruct {
        vis: item.vis.clone(),
        name: item.ident.clone(),
        generics: item.generics.clone(),
        fields,
    })
}
