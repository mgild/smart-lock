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
    let guard_doc = format!(
        "Guard holding acquired locks for [`{lock}`].\n\n\
         Access fields via `guard.field_name` — uses `Deref`/`DerefMut` based on the lock mode:\n\
         - **`WriteLocked`**: `*guard.field` for read, `*guard.field = val` for write\n\
         - **`ReadLocked`**: `*guard.field` for read only (mutation is a compile error)\n\
         - **`UpgradeLocked`**: `*guard.field` for read, `.upgrade_field().await` to promote to write\n\
         - **`Unlocked`**: compile error on any access\n\n\
         All locks are released when the guard is dropped.",
        lock = format!("{}Lock", struct_name_str)
    );

    let n = parsed.fields.len();
    let generic_names: Vec<syn::Ident> = (0..n)
        .map(|i| format_ident!("F{}", i))
        .collect();

    let guard_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            let f = &generic_names[i];
            quote! {
                pub #name: smart_lock::FieldGuard<'a, #ty, #f>,
            }
        })
        .collect();

    let all_unlocked: Vec<proc_macro2::TokenStream> = (0..n)
        .map(|_| quote!(smart_lock::Unlocked))
        .collect();

    // --- Upgrade/downgrade impl blocks per field ---
    let mut transition_impls = Vec::new();

    for (i, field) in parsed.fields.iter().enumerate() {
        let field_name = &field.name;
        let field_name_str = field_name.to_string();
        let upgrade_method = format_ident!("upgrade_{}", field_name);
        let downgrade_method = format_ident!("downgrade_{}", field_name);

        let upgrade_doc = format!(
            "Atomically upgrade `{}` from upgradable read to exclusive write.\n\n\
             Waits for all other readers to drain. Other fields remain locked as before.",
            field_name_str
        );
        let downgrade_from_upgrade_doc = format!(
            "Atomically downgrade `{}` from upgradable read to shared read.\n\n\
             Releases the upgrade slot, allowing other tasks to acquire upgradable locks. Synchronous (no `.await`).",
            field_name_str
        );
        let downgrade_from_write_doc = format!(
            "Atomically downgrade `{}` from exclusive write to shared read.\n\n\
             Immediately allows other readers. Synchronous (no `.await`).",
            field_name_str
        );

        let free_generics: Vec<&syn::Ident> = generic_names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, name)| name)
            .collect();

        let other_fields: Vec<proc_macro2::TokenStream> = parsed.fields
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| {
                let n = &f.name;
                quote!(#n: self.#n,)
            })
            .collect();

        let make_params = |mode: proc_macro2::TokenStream| -> Vec<proc_macro2::TokenStream> {
            (0..n).map(|j| {
                if j == i { mode.clone() } else { let f = &generic_names[j]; quote!(#f) }
            }).collect()
        };

        let upgrade_input = make_params(quote!(smart_lock::UpgradeLocked));
        let write_output = make_params(quote!(smart_lock::WriteLocked));
        let read_output = make_params(quote!(smart_lock::ReadLocked));
        let write_input = make_params(quote!(smart_lock::WriteLocked));

        if has_generics {
            transition_impls.push(quote! {
                impl<'a, #(#impl_params),*, #(#free_generics),*> #guard_name<'a, #(#bare),*, #(#upgrade_input),*> #where_clause {
                    #[doc = #upgrade_doc]
                    #vis async fn #upgrade_method(self) -> #guard_name<'a, #(#bare),*, #(#write_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.upgrade().await,
                            #(#other_fields)*
                        }
                    }

                    #[doc = #downgrade_from_upgrade_doc]
                    #vis fn #downgrade_method(self) -> #guard_name<'a, #(#bare),*, #(#read_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.downgrade(),
                            #(#other_fields)*
                        }
                    }
                }
            });

            transition_impls.push(quote! {
                impl<'a, #(#impl_params),*, #(#free_generics),*> #guard_name<'a, #(#bare),*, #(#write_input),*> #where_clause {
                    #[doc = #downgrade_from_write_doc]
                    #vis fn #downgrade_method(self) -> #guard_name<'a, #(#bare),*, #(#read_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.downgrade(),
                            #(#other_fields)*
                        }
                    }
                }
            });
        } else {
            transition_impls.push(quote! {
                impl<'a, #(#free_generics),*> #guard_name<'a, #(#upgrade_input),*> {
                    #[doc = #upgrade_doc]
                    #vis async fn #upgrade_method(self) -> #guard_name<'a, #(#write_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.upgrade().await,
                            #(#other_fields)*
                        }
                    }

                    #[doc = #downgrade_from_upgrade_doc]
                    #vis fn #downgrade_method(self) -> #guard_name<'a, #(#read_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.downgrade(),
                            #(#other_fields)*
                        }
                    }
                }
            });

            transition_impls.push(quote! {
                impl<'a, #(#free_generics),*> #guard_name<'a, #(#write_input),*> {
                    #[doc = #downgrade_from_write_doc]
                    #vis fn #downgrade_method(self) -> #guard_name<'a, #(#read_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.downgrade(),
                            #(#other_fields)*
                        }
                    }
                }
            });
        }
    }

    // --- relock() method ---
    let lock_bounds: Vec<proc_macro2::TokenStream> = generic_names
        .iter()
        .map(|f| quote!(#f: smart_lock::LockMode))
        .collect();

    let relock_builder_ty = if has_generics {
        quote!(#builder_name<'a, #(#bare),*, #(#all_unlocked),*>)
    } else {
        quote!(#builder_name<'a, #(#all_unlocked),*>)
    };

    let relock_impl = if has_generics {
        quote! {
            impl<'a, #(#impl_params),*, #(#lock_bounds),*> #guard_name<'a, #(#bare),*, #(#generic_names),*> #where_clause {
                /// Drop all held locks and return a fresh builder for the same lock.
                ///
                /// This lets you re-acquire a different set of fields without dropping
                /// the lock reference.
                ///
                /// **Warning:** There is a moment between dropping the old locks and
                /// acquiring new ones where no locks are held. Other tasks may modify
                /// fields during this gap. Do not assume atomicity across a `relock()`.
                #vis fn relock(self) -> #relock_builder_ty {
                    let lock = self.lock;
                    drop(self);
                    #builder_name { lock, _marker: std::marker::PhantomData }
                }
            }
        }
    } else {
        quote! {
            impl<'a, #(#lock_bounds),*> #guard_name<'a, #(#generic_names),*> {
                /// Drop all held locks and return a fresh builder for the same lock.
                ///
                /// This lets you re-acquire a different set of fields without dropping
                /// the lock reference.
                ///
                /// **Warning:** There is a moment between dropping the old locks and
                /// acquiring new ones where no locks are held. Other tasks may modify
                /// fields during this gap. Do not assume atomicity across a `relock()`.
                #vis fn relock(self) -> #relock_builder_ty {
                    let lock = self.lock;
                    drop(self);
                    #builder_name { lock, _marker: std::marker::PhantomData }
                }
            }
        }
    };

    // Guard struct definition
    let guard_struct = if has_generics {
        quote! {
            #[doc = #guard_doc]
            #vis struct #guard_name<'a, #(#impl_params),*, #(#generic_names),*> #where_clause {
                #[doc(hidden)]
                lock: &'a #lock_name #ty_generics,
                #(#guard_fields)*
            }
        }
    } else {
        quote! {
            #[doc = #guard_doc]
            #vis struct #guard_name<'a, #(#generic_names),*> {
                #[doc(hidden)]
                lock: &'a #lock_name,
                #(#guard_fields)*
            }
        }
    };

    quote! {
        #guard_struct
        #(#transition_impls)*
        #relock_impl
    }
}
