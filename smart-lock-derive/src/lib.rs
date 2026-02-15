use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemStruct};

mod parse;
mod gen_field_ids;
mod gen_lock;
mod gen_builder;
mod gen_guard;
mod gen_from;
mod gen_impls;

#[proc_macro_attribute]
pub fn smart_lock(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item_struct = parse_macro_input!(item as ItemStruct);
    let attr2 = proc_macro2::TokenStream::from(attr);

    let parsed = match parse::parse(attr2, &item_struct) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into(),
    };

    let original = &item_struct;
    let field_ids = gen_field_ids::generate(&parsed);
    let lock = gen_lock::generate(&parsed);
    let guard = gen_guard::generate(&parsed);
    let builder = gen_builder::generate(&parsed);
    let from = gen_from::generate(&parsed);
    let impls = gen_impls::generate(&parsed);

    let expanded = quote::quote! {
        #original
        #field_ids
        #lock
        #guard
        #builder
        #from
        #impls
    };

    expanded.into()
}
