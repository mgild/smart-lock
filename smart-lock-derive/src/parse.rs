use quote::quote;
use syn::{Attribute, Fields, Generics, Ident, ItemStruct, Type, Visibility};

pub struct ParsedField {
    pub name: Ident,
    pub ty: Type,
    #[allow(dead_code)]
    pub vis: Visibility,
    pub attrs: Vec<Attribute>,
    pub no_lock: bool,
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
        self.generics
            .params
            .iter()
            .map(|p| match p {
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
            })
            .collect()
    }

    /// Returns full generic params (with bounds) for use in impl parameter lists.
    pub fn impl_generic_params(&self) -> Vec<proc_macro2::TokenStream> {
        self.generics.params.iter().map(|p| quote!(#p)).collect()
    }

    /// Bare struct generic params with trailing comma, or empty.
    /// Use in type applications: `<'a, #bare_prefix #(#field_generics),*>`
    pub fn bare_prefix(&self) -> proc_macro2::TokenStream {
        let bare = self.bare_generic_params();
        if bare.is_empty() {
            quote!()
        } else {
            quote!(#(#bare),*,)
        }
    }

    /// Full struct generic params (with bounds) with trailing comma, or empty.
    /// Use in impl headers: `impl<'a, #impl_prefix #(#field_generics),*>`
    pub fn impl_prefix(&self) -> proc_macro2::TokenStream {
        let params = self.impl_generic_params();
        if params.is_empty() {
            quote!()
        } else {
            quote!(#(#params),*,)
        }
    }

    /// The where clause (if any) from the original struct.
    pub fn where_clause(&self) -> Option<&syn::WhereClause> {
        self.generics.where_clause.as_ref()
    }

    /// Type-application generics for the Lock struct: `<T, U>` or empty.
    pub fn ty_generics(&self) -> proc_macro2::TokenStream {
        let bare = self.bare_generic_params();
        if bare.is_empty() {
            quote!()
        } else {
            quote!(<#(#bare),*>)
        }
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
        .map(|f| {
            let no_lock = f.attrs.iter().any(|a| a.path().is_ident("no_lock"));
            let attrs: Vec<Attribute> = f
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("no_lock"))
                .cloned()
                .collect();
            ParsedField {
                name: f.ident.clone().unwrap(),
                ty: f.ty.clone(),
                vis: f.vis.clone(),
                attrs,
                no_lock,
            }
        })
        .collect();

    Ok(ParsedStruct {
        vis: item.vis.clone(),
        name: item.ident.clone(),
        generics: item.generics.clone(),
        fields,
    })
}
