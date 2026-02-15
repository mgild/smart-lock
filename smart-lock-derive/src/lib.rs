use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemStruct};

mod gen_builder;
mod gen_from;
mod gen_guard;
mod gen_lock;
mod parse;

#[proc_macro_attribute]
pub fn smart_lock(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(item as ItemStruct);
    let attr2 = proc_macro2::TokenStream::from(attr);

    let parsed = match parse::parse(attr2, &item_struct) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into(),
    };

    let mut clean_struct = item_struct.clone();
    if let syn::Fields::Named(ref mut fields) = clean_struct.fields {
        for field in &mut fields.named {
            field.attrs.retain(|a| !a.path().is_ident("no_lock"));
        }
    }
    let original = &clean_struct;
    let lock = gen_lock::generate(&parsed);
    let guard = gen_guard::generate(&parsed);
    let builder = gen_builder::generate(&parsed);
    let from = gen_from::generate(&parsed);

    let expanded = quote::quote! {
        #original
        #lock
        #guard
        #builder
        #from
    };

    expanded.into()
}
