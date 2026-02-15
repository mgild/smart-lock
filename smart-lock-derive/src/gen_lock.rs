use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let vis = &parsed.vis;
    let struct_name = &parsed.name;
    let lock_name = format_ident!("{}Lock", struct_name);
    let builder_name = format_ident!("{}LockBuilder", struct_name);
    let guard_name = format_ident!("{}LockGuard", struct_name);

    let (impl_generics, ty_generics, where_clause) = parsed.generics.split_for_impl();
    let bare = parsed.bare_generic_params();
    let has_generics = parsed.has_generics();

    let n = parsed.fields.len();

    let lock_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let attrs = &field.attrs;
            quote! {
                #(#attrs)*
                #name: smart_lock::RwLock<#ty>,
            }
        })
        .collect();

    let new_params: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            quote! { #name: #ty }
        })
        .collect();

    let new_inits: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            quote! {
                #name: smart_lock::RwLock::new(#name),
            }
        })
        .collect();

    let all_unlocked: Vec<proc_macro2::TokenStream> = (0..n)
        .map(|_| quote!(smart_lock::Unlocked))
        .collect();
    let all_read: Vec<proc_macro2::TokenStream> = (0..n)
        .map(|_| quote!(smart_lock::ReadLocked))
        .collect();
    let all_write: Vec<proc_macro2::TokenStream> = (0..n)
        .map(|_| quote!(smart_lock::WriteLocked))
        .collect();

    let lock_all_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            quote! {
                let #name = smart_lock::FieldGuard::<'_, #ty, smart_lock::ReadLocked>::acquire(&self.#name).await;
            }
        })
        .collect();

    let lock_all_mut_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            quote! {
                let #name = smart_lock::FieldGuard::<'_, #ty, smart_lock::WriteLocked>::acquire(&self.#name).await;
            }
        })
        .collect();

    let field_names: Vec<&syn::Ident> = parsed.fields.iter().map(|f| &f.name).collect();

    let per_field_accessors: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let read_method = format_ident!("read_{}", name);
            let write_method = format_ident!("write_{}", name);
            let try_read_method = format_ident!("try_read_{}", name);
            let try_write_method = format_ident!("try_write_{}", name);
            let upgrade_method = format_ident!("upgrade_{}", name);
            let try_upgrade_method = format_ident!("try_upgrade_{}", name);

            quote! {
                #vis async fn #read_method(&self) -> smart_lock::RwLockReadGuard<'_, #ty> {
                    self.#name.read().await
                }

                #vis async fn #write_method(&self) -> smart_lock::RwLockWriteGuard<'_, #ty> {
                    self.#name.write().await
                }

                #vis fn #try_read_method(&self) -> Option<smart_lock::RwLockReadGuard<'_, #ty>> {
                    self.#name.try_read()
                }

                #vis fn #try_write_method(&self) -> Option<smart_lock::RwLockWriteGuard<'_, #ty>> {
                    self.#name.try_write()
                }

                #vis async fn #upgrade_method(&self) -> smart_lock::RwLockUpgradableReadGuard<'_, #ty> {
                    self.#name.upgradable_read().await
                }

                #vis fn #try_upgrade_method(&self) -> Option<smart_lock::RwLockUpgradableReadGuard<'_, #ty>> {
                    self.#name.try_upgradable_read()
                }
            }
        })
        .collect();

    let into_inner_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            quote! { #name: self.#name.into_inner(), }
        })
        .collect();

    let get_mut_accessors: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let method = format_ident!("get_mut_{}", name);
            quote! {
                #vis fn #method(&mut self) -> &mut #ty {
                    self.#name.get_mut()
                }
            }
        })
        .collect();

    // Use bare generics in type applications, full generics in impl blocks
    let builder_ty = if has_generics {
        quote!(#builder_name<'_, #(#bare),*, #(#all_unlocked),*>)
    } else {
        quote!(#builder_name<'_, #(#all_unlocked),*>)
    };

    let guard_all_read_ty = if has_generics {
        quote!(#guard_name<'_, #(#bare),*, #(#all_read),*>)
    } else {
        quote!(#guard_name<'_, #(#all_read),*>)
    };

    let guard_all_write_ty = if has_generics {
        quote!(#guard_name<'_, #(#bare),*, #(#all_write),*>)
    } else {
        quote!(#guard_name<'_, #(#all_write),*>)
    };

    quote! {
        #vis struct #lock_name #impl_generics #where_clause {
            #(#lock_fields)*
        }

        impl #impl_generics #lock_name #ty_generics #where_clause {
            #vis fn new(#(#new_params),*) -> Self {
                Self {
                    #(#new_inits)*
                }
            }

            #vis fn builder(&self) -> #builder_ty {
                #builder_name { lock: self, _marker: std::marker::PhantomData }
            }

            #vis async fn lock_all(&self) -> #guard_all_read_ty {
                #(#lock_all_fields)*
                #guard_name { lock: self, #(#field_names),* }
            }

            #vis async fn lock_all_mut(&self) -> #guard_all_write_ty {
                #(#lock_all_mut_fields)*
                #guard_name { lock: self, #(#field_names),* }
            }

            #vis fn into_inner(self) -> #struct_name #ty_generics {
                #struct_name {
                    #(#into_inner_fields)*
                }
            }

            #(#per_field_accessors)*

            #(#get_mut_accessors)*
        }
    }
}
