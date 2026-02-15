use crate::parse::ParsedStruct;
use quote::{quote, format_ident};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let vis = &parsed.vis;
    let lock_name = format_ident!("{}Lock", &parsed.name);
    let builder_name = format_ident!("{}LockBuilder", &parsed.name);
    let guard_name = format_ident!("{}LockGuard", &parsed.name);

    let (_, ty_generics, where_clause) = parsed.generics.split_for_impl();
    let impl_params = parsed.impl_generic_params();
    let bare = parsed.bare_generic_params();
    let has_generics = parsed.has_generics();

    let struct_name_str = parsed.name.to_string();
    let builder_doc = format!(
        "Type-state builder for selecting which fields of [`{lock}`] to lock.\n\n\
         Each field starts as `Unlocked`. Call `.read_field()`, `.write_field()`, or \
         `.upgrade_field()` to select the lock mode, then `.lock().await` to acquire all \
         selected locks atomically.\n\n\
         Locks are acquired in field declaration order to prevent deadlocks. \
         A field can only be locked once (calling `.write_x()` on an already-locked field \
         is a compile error).",
        lock = format!("{}Lock", struct_name_str)
    );

    let n = parsed.fields.len();
    let generic_names: Vec<syn::Ident> = (0..n)
        .map(|i| format_ident!("F{}", i))
        .collect();

    // --- Builder struct definition ---
    let struct_def = if has_generics {
        quote! {
            #[doc = #builder_doc]
            #vis struct #builder_name<'a, #(#impl_params),*, #(#generic_names),*> #where_clause {
                lock: &'a #lock_name #ty_generics,
                _marker: std::marker::PhantomData<(#(#generic_names),*)>,
            }
        }
    } else {
        quote! {
            #[doc = #builder_doc]
            #vis struct #builder_name<'a, #(#generic_names),*> {
                lock: &'a #lock_name,
                _marker: std::marker::PhantomData<(#(#generic_names),*)>,
            }
        }
    };

    // --- Per-field impl blocks ---
    let mut field_impls = Vec::new();

    for (i, field) in parsed.fields.iter().enumerate() {
        let field_name = &field.name;
        let field_name_str = field_name.to_string();
        let write_method = format_ident!("write_{}", field_name);
        let read_method = format_ident!("read_{}", field_name);
        let upgrade_method = format_ident!("upgrade_{}", field_name);

        let write_doc = format!("Request exclusive write access to `{}`.", field_name_str);
        let read_doc = format!("Request shared read access to `{}`.", field_name_str);
        let upgrade_doc = format!("Request upgradable read access to `{}`. Can be atomically upgraded to write access later via `.upgrade_{}().await` on the guard.", field_name_str, field_name_str);

        let free_generics: Vec<&syn::Ident> = generic_names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, name)| name)
            .collect();

        let input_params: Vec<proc_macro2::TokenStream> = (0..n)
            .map(|j| {
                if j == i { quote!(smart_lock::Unlocked) } else { let f = &generic_names[j]; quote!(#f) }
            })
            .collect();

        let write_params: Vec<proc_macro2::TokenStream> = (0..n)
            .map(|j| {
                if j == i { quote!(smart_lock::WriteLocked) } else { let f = &generic_names[j]; quote!(#f) }
            })
            .collect();

        let read_params: Vec<proc_macro2::TokenStream> = (0..n)
            .map(|j| {
                if j == i { quote!(smart_lock::ReadLocked) } else { let f = &generic_names[j]; quote!(#f) }
            })
            .collect();

        let upgrade_params: Vec<proc_macro2::TokenStream> = (0..n)
            .map(|j| {
                if j == i { quote!(smart_lock::UpgradeLocked) } else { let f = &generic_names[j]; quote!(#f) }
            })
            .collect();

        if has_generics {
            field_impls.push(quote! {
                impl<'a, #(#impl_params),*, #(#free_generics),*> #builder_name<'a, #(#bare),*, #(#input_params),*> #where_clause {
                    #[doc = #write_doc]
                    #vis fn #write_method(self) -> #builder_name<'a, #(#bare),*, #(#write_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }

                    #[doc = #read_doc]
                    #vis fn #read_method(self) -> #builder_name<'a, #(#bare),*, #(#read_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }

                    #[doc = #upgrade_doc]
                    #vis fn #upgrade_method(self) -> #builder_name<'a, #(#bare),*, #(#upgrade_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }
                }
            });
        } else {
            field_impls.push(quote! {
                impl<'a, #(#free_generics),*> #builder_name<'a, #(#input_params),*> {
                    #[doc = #write_doc]
                    #vis fn #write_method(self) -> #builder_name<'a, #(#write_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }

                    #[doc = #read_doc]
                    #vis fn #read_method(self) -> #builder_name<'a, #(#read_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }

                    #[doc = #upgrade_doc]
                    #vis fn #upgrade_method(self) -> #builder_name<'a, #(#upgrade_params),*> {
                        #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                    }
                }
            });
        }
    }

    // --- lock() method ---
    let lock_bounds: Vec<proc_macro2::TokenStream> = generic_names
        .iter()
        .map(|f| quote!(#f: smart_lock::LockMode))
        .collect();

    let lock_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            let f = &generic_names[i];
            quote! {
                let #name = if <#f as smart_lock::LockMode>::MODE == smart_lock::LockModeKind::None {
                    smart_lock::FieldGuard::<'_, #ty, #f>::unlocked()
                } else {
                    smart_lock::FieldGuard::<'_, #ty, #f>::acquire(&self.lock.#name).await
                };
            }
        })
        .collect();

    let field_names: Vec<&syn::Ident> = parsed.fields.iter().map(|f| &f.name).collect();

    let lock_impl = if has_generics {
        quote! {
            impl<'a, #(#impl_params),*, #(#lock_bounds),*> #builder_name<'a, #(#bare),*, #(#generic_names),*> #where_clause {
                /// Acquire all requested locks and return the guard.
                ///
                /// Locks are acquired in field declaration order (not call order) to prevent deadlocks.
                /// Unlocked fields are skipped with zero overhead.
                #vis async fn lock(self) -> #guard_name<'a, #(#bare),*, #(#generic_names),*> {
                    #(#lock_fields)*
                    #guard_name { lock: self.lock, #(#field_names),* }
                }
            }
        }
    } else {
        quote! {
            impl<'a, #(#lock_bounds),*> #builder_name<'a, #(#generic_names),*> {
                /// Acquire all requested locks and return the guard.
                ///
                /// Locks are acquired in field declaration order (not call order) to prevent deadlocks.
                /// Unlocked fields are skipped with zero overhead.
                #vis async fn lock(self) -> #guard_name<'a, #(#generic_names),*> {
                    #(#lock_fields)*
                    #guard_name { lock: self.lock, #(#field_names),* }
                }
            }
        }
    };

    quote! {
        #struct_def
        #(#field_impls)*
        #lock_impl
    }
}
