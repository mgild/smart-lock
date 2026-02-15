use crate::parse::ParsedStruct;
use quote::{format_ident, quote};

pub fn generate(parsed: &ParsedStruct) -> proc_macro2::TokenStream {
    let vis = &parsed.vis;
    let lock_name = format_ident!("{}Lock", &parsed.name);
    let builder_name = format_ident!("{}LockBuilder", &parsed.name);
    let guard_name = format_ident!("{}LockGuard", &parsed.name);

    let impl_prefix = parsed.impl_prefix();
    let bare_prefix = parsed.bare_prefix();
    let ty_generics = parsed.ty_generics();
    let where_clause = parsed.where_clause();

    let lock_name_str = format!("{}Lock", parsed.name);
    let builder_doc = format!(
        "Type-state builder for selecting which fields of [`{lock_name_str}`] to lock.\n\n\
         Each field starts as `Unlocked`. Call `.read_field()`, `.write_field()`, or \
         `.upgrade_field()` to select the lock mode, then `.lock().await` to acquire all \
         selected locks atomically.\n\n\
         Locks are acquired in field declaration order to prevent deadlocks. \
         A field can only be locked once (calling `.write_x()` on an already-locked field \
         is a compile error)."
    );

    // Map field index → generic index (None for no_lock fields)
    let field_to_generic: Vec<Option<usize>> = {
        let mut gi = 0;
        parsed
            .fields
            .iter()
            .map(|f| {
                if f.no_lock {
                    None
                } else {
                    let idx = gi;
                    gi += 1;
                    Some(idx)
                }
            })
            .collect()
    };

    let locked_count = field_to_generic.iter().filter(|g| g.is_some()).count();
    let generic_names: Vec<syn::Ident> =
        (0..locked_count).map(|i| format_ident!("F{}", i)).collect();

    // --- Builder struct definition ---
    let struct_def = quote! {
        #[doc = #builder_doc]
        #[must_use = "builder does nothing until .lock().await is called"]
        #vis struct #builder_name<'a, #impl_prefix #(#generic_names),*> #where_clause {
            lock: &'a #lock_name #ty_generics,
            _marker: std::marker::PhantomData<(#(#generic_names),*)>,
        }
    };

    // --- Per-field impl blocks (locked fields only) ---
    let mut field_impls = Vec::new();

    for (i, field) in parsed.fields.iter().enumerate() {
        if field.no_lock {
            continue;
        }

        let gi = field_to_generic[i].unwrap();
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
            .filter(|(j, _)| *j != gi)
            .map(|(_, name)| name)
            .collect();

        let input_params: Vec<proc_macro2::TokenStream> = (0..locked_count)
            .map(|j| {
                if j == gi {
                    quote!(smart_lock::Unlocked)
                } else {
                    let f = &generic_names[j];
                    quote!(#f)
                }
            })
            .collect();

        let write_params: Vec<proc_macro2::TokenStream> = (0..locked_count)
            .map(|j| {
                if j == gi {
                    quote!(smart_lock::WriteLocked)
                } else {
                    let f = &generic_names[j];
                    quote!(#f)
                }
            })
            .collect();

        let read_params: Vec<proc_macro2::TokenStream> = (0..locked_count)
            .map(|j| {
                if j == gi {
                    quote!(smart_lock::ReadLocked)
                } else {
                    let f = &generic_names[j];
                    quote!(#f)
                }
            })
            .collect();

        let upgrade_params: Vec<proc_macro2::TokenStream> = (0..locked_count)
            .map(|j| {
                if j == gi {
                    quote!(smart_lock::UpgradeLocked)
                } else {
                    let f = &generic_names[j];
                    quote!(#f)
                }
            })
            .collect();

        field_impls.push(quote! {
            impl<'a, #impl_prefix #(#free_generics),*> #builder_name<'a, #bare_prefix #(#input_params),*> #where_clause {
                #[doc = #write_doc]
                #vis fn #write_method(self) -> #builder_name<'a, #bare_prefix #(#write_params),*> {
                    #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                }

                #[doc = #read_doc]
                #vis fn #read_method(self) -> #builder_name<'a, #bare_prefix #(#read_params),*> {
                    #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                }

                #[doc = #upgrade_doc]
                #vis fn #upgrade_method(self) -> #builder_name<'a, #bare_prefix #(#upgrade_params),*> {
                    #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
                }
            }
        });
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
            if field.no_lock {
                quote! { let #name = &self.lock.#name; }
            } else {
                let gi = field_to_generic[i].unwrap();
                let f = &generic_names[gi];
                quote! {
                    let #name = if <#f as smart_lock::LockMode>::MODE == smart_lock::LockModeKind::None {
                        smart_lock::FieldGuard::<'_, #ty, #f>::unlocked()
                    } else {
                        smart_lock::FieldGuard::<'_, #ty, #f>::acquire(&self.lock.#name).await
                    };
                }
            }
        })
        .collect();

    let try_lock_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            if field.no_lock {
                quote! { let #name = &self.lock.#name; }
            } else {
                let gi = field_to_generic[i].unwrap();
                let f = &generic_names[gi];
                quote! {
                    let #name = if <#f as smart_lock::LockMode>::MODE == smart_lock::LockModeKind::None {
                        smart_lock::FieldGuard::<'_, #ty, #f>::unlocked()
                    } else {
                        smart_lock::FieldGuard::<'_, #ty, #f>::try_acquire(&self.lock.#name)?
                    };
                }
            }
        })
        .collect();

    let field_names: Vec<&syn::Ident> = parsed.fields.iter().map(|f| &f.name).collect();

    let lock_impl = quote! {
        impl<'a, #impl_prefix #(#lock_bounds),*> #builder_name<'a, #bare_prefix #(#generic_names),*> #where_clause {
            /// Acquire all requested locks and return the guard.
            ///
            /// Locks are acquired in field declaration order (not call order) to prevent deadlocks.
            /// Unlocked fields are skipped with zero overhead.
            #vis async fn lock(self) -> #guard_name<'a, #bare_prefix #(#generic_names),*> {
                #(#lock_fields)*
                #guard_name { lock: self.lock, #(#field_names),* }
            }

            /// Try to acquire all requested locks without blocking.
            ///
            /// Returns `None` if any lock is currently held in a conflicting mode.
            /// On failure, all already-acquired locks are released (the partially-built
            /// guard is dropped). Locks are attempted in field declaration order.
            #vis fn try_lock(self) -> Option<#guard_name<'a, #bare_prefix #(#generic_names),*>> {
                #(#try_lock_fields)*
                Some(#guard_name { lock: self.lock, #(#field_names),* })
            }
        }
    };

    // --- lock_rest_read() / try_lock_rest_read() ---
    let rest_read_bounds: Vec<proc_macro2::TokenStream> = generic_names
        .iter()
        .map(|f| quote!(#f: smart_lock::DefaultRead))
        .collect();

    let rest_read_output_generics: Vec<proc_macro2::TokenStream> = generic_names
        .iter()
        .map(|f| quote!(<#f as smart_lock::DefaultRead>::Output))
        .collect();

    let rest_read_lock_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            if field.no_lock {
                quote! { let #name = &self.lock.#name; }
            } else {
                let gi = field_to_generic[i].unwrap();
                let f = &generic_names[gi];
                quote! {
                    let #name = smart_lock::FieldGuard::<'_, #ty, <#f as smart_lock::DefaultRead>::Output>::acquire(&self.lock.#name).await;
                }
            }
        })
        .collect();

    let rest_read_try_lock_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            if field.no_lock {
                quote! { let #name = &self.lock.#name; }
            } else {
                let gi = field_to_generic[i].unwrap();
                let f = &generic_names[gi];
                quote! {
                    let #name = smart_lock::FieldGuard::<'_, #ty, <#f as smart_lock::DefaultRead>::Output>::try_acquire(&self.lock.#name)?;
                }
            }
        })
        .collect();

    let rest_read_impl = quote! {
        impl<'a, #impl_prefix #(#rest_read_bounds),*> #builder_name<'a, #bare_prefix #(#generic_names),*> #where_clause {
            /// Acquire locks for all fields, filling any `Unlocked` fields with read locks.
            ///
            /// Fields already set to `WriteLocked` or `UpgradeLocked` keep their mode.
            /// Fields left `Unlocked` (not explicitly selected) become `ReadLocked`.
            ///
            /// This is a shorthand for when you want to write a few fields and read the rest,
            /// without listing every field in the builder.
            #vis async fn lock_rest_read(self) -> #guard_name<'a, #bare_prefix #(#rest_read_output_generics),*> {
                #(#rest_read_lock_fields)*
                #guard_name { lock: self.lock, #(#field_names),* }
            }

            /// Try to acquire locks for all fields without blocking, filling `Unlocked` fields
            /// with read locks.
            ///
            /// Returns `None` if any lock is currently held in a conflicting mode.
            /// On failure, all already-acquired locks are released.
            #vis fn try_lock_rest_read(self) -> Option<#guard_name<'a, #bare_prefix #(#rest_read_output_generics),*>> {
                #(#rest_read_try_lock_fields)*
                Some(#guard_name { lock: self.lock, #(#field_names),* })
            }
        }
    };

    quote! {
        #struct_def
        #(#field_impls)*
        #lock_impl
        #rest_read_impl
    }
}
