use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let vis = &parsed.vis;
    let struct_name = &parsed.name;
    let lock_name = format_ident!("{}Lock", struct_name);
    let builder_name = format_ident!("{}LockBuilder", struct_name);
    let guard_name = format_ident!("{}LockGuard", struct_name);

    let impl_prefix = parsed.impl_prefix();
    let bare_prefix = parsed.bare_prefix();
    let ty_generics = parsed.ty_generics();
    let where_clause = parsed.where_clause();

    let struct_name_str = struct_name.to_string();
    let lock_doc = format!(
        "Per-field async `RwLock` wrapper for [`{}`].\n\n\
         Each field is independently lockable. Use [`.builder()`](Self::builder) to select \
         which fields to lock and how, or [`.lock_all()`](Self::lock_all) / \
         [`.lock_all_mut()`](Self::lock_all_mut) for convenience.\n\n\
         Created by `#[smart_lock]` on `{}`.",
        struct_name_str, struct_name_str
    );

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
            let name_str = name.to_string();
            let read_method = format_ident!("read_{}", name);
            let write_method = format_ident!("write_{}", name);
            let try_read_method = format_ident!("try_read_{}", name);
            let try_write_method = format_ident!("try_write_{}", name);
            let upgrade_method = format_ident!("upgrade_{}", name);
            let try_upgrade_method = format_ident!("try_upgrade_{}", name);

            let read_doc = format!("Acquire a shared read lock on `{}`.", name_str);
            let write_doc = format!("Acquire an exclusive write lock on `{}`.", name_str);
            let try_read_doc = format!("Try to acquire a shared read lock on `{}`. Returns `None` if the lock is held exclusively.", name_str);
            let try_write_doc = format!("Try to acquire an exclusive write lock on `{}`. Returns `None` if the lock is held.", name_str);
            let upgrade_doc = format!("Acquire an upgradable read lock on `{}`. Can be atomically upgraded to a write lock later.", name_str);
            let try_upgrade_doc = format!("Try to acquire an upgradable read lock on `{}`. Returns `None` if another upgradable or write lock is held.", name_str);
            quote! {
                #[doc = #read_doc]
                #vis async fn #read_method(&self) -> smart_lock::RwLockReadGuard<'_, #ty> {
                    self.#name.read().await
                }

                #[doc = #write_doc]
                #vis async fn #write_method(&self) -> smart_lock::RwLockWriteGuard<'_, #ty> {
                    self.#name.write().await
                }

                #[doc = #try_read_doc]
                #vis fn #try_read_method(&self) -> Option<smart_lock::RwLockReadGuard<'_, #ty>> {
                    self.#name.try_read()
                }

                #[doc = #try_write_doc]
                #vis fn #try_write_method(&self) -> Option<smart_lock::RwLockWriteGuard<'_, #ty>> {
                    self.#name.try_write()
                }

                #[doc = #upgrade_doc]
                #vis async fn #upgrade_method(&self) -> smart_lock::RwLockUpgradableReadGuard<'_, #ty> {
                    self.#name.upgradable_read().await
                }

                #[doc = #try_upgrade_doc]
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

    let into_inner_doc = format!(
        "Consume the lock and return the original [`{name}`] with all field values.\n\n\
         When the lock is behind an `Arc`, unwrap it first:\n\
         ```ignore\n\
         let inner = Arc::try_unwrap(arc).expect(\"other refs exist\").into_inner();\n\
         ```",
        name = struct_name_str
    );

    let get_mut_accessors: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .map(|field| {
            let name = &field.name;
            let ty = &field.ty;
            let name_str = name.to_string();
            let method = format_ident!("get_mut_{}", name);
            let get_mut_doc = format!("Get a mutable reference to `{}` without locking. Requires `&mut self`, guaranteeing exclusive access.", name_str);
            quote! {
                #[doc = #get_mut_doc]
                #vis fn #method(&mut self) -> &mut #ty {
                    self.#name.get_mut()
                }
            }
        })
        .collect();

    // Static assertion that the Lock type is Send + Sync.
    // Uses a hidden const fn that requires Send + Sync bounds.
    let assert_name = format_ident!("_assert_{}_send_sync", lock_name);

    quote! {
        #[doc = #lock_doc]
        #vis struct #lock_name #ty_generics #where_clause {
            #(#lock_fields)*
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        const _: () = {
            fn #assert_name<#impl_prefix>() #where_clause {
                fn _require_send_sync<T: Send + Sync>() {}
                _require_send_sync::<#lock_name #ty_generics>();
            }
        };

        impl<#impl_prefix> #lock_name #ty_generics #where_clause {
            /// Create a new lock wrapping each field in an `RwLock`.
            #vis fn new(#(#new_params),*) -> Self {
                Self {
                    #(#new_inits)*
                }
            }

            /// Start building a lock request. Chain `.read_field()`, `.write_field()`,
            /// or `.upgrade_field()` calls, then `.lock().await` to acquire.
            ///
            /// Locks are acquired in field declaration order to prevent deadlocks.
            #vis fn builder(&self) -> #builder_name<'_, #bare_prefix #(#all_unlocked),*> {
                #builder_name { lock: self, _marker: std::marker::PhantomData }
            }

            /// Read-lock all fields. Convenience for `builder().read_a().read_b()...lock().await`.
            #vis async fn lock_all(&self) -> #guard_name<'_, #bare_prefix #(#all_read),*> {
                #(#lock_all_fields)*
                #guard_name { lock: self, #(#field_names),* }
            }

            /// Write-lock all fields. Convenience for `builder().write_a().write_b()...lock().await`.
            #vis async fn lock_all_mut(&self) -> #guard_name<'_, #bare_prefix #(#all_write),*> {
                #(#lock_all_mut_fields)*
                #guard_name { lock: self, #(#field_names),* }
            }

            #[doc = #into_inner_doc]
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
