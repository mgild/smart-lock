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
    let guard_doc = format!(
        "Guard holding acquired locks for [`{lock_name_str}`].\n\n\
         Access fields via `guard.field_name` — uses `Deref`/`DerefMut` based on the lock mode:\n\
         - **`WriteLocked`**: `*guard.field` for read, `*guard.field = val` for write\n\
         - **`ReadLocked`**: `*guard.field` for read only (mutation is a compile error)\n\
         - **`UpgradeLocked`**: `*guard.field` for read, `.upgrade_field().await` to promote to write\n\
         - **`Unlocked`**: compile error on any access\n\
         - **`#[no_lock]`**: always accessible as `&T` (no locking needed)\n\n\
         All locks are released when the guard is dropped."
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

    let guard_fields: Vec<proc_macro2::TokenStream> = parsed
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let name = &field.name;
            let ty = &field.ty;
            if field.no_lock {
                quote! { pub #name: &'a #ty, }
            } else {
                let gi = field_to_generic[i].unwrap();
                let f = &generic_names[gi];
                quote! { pub #name: smart_lock::FieldGuard<'a, #ty, #f>, }
            }
        })
        .collect();

    let all_unlocked: Vec<proc_macro2::TokenStream> = (0..locked_count)
        .map(|_| quote!(smart_lock::Unlocked))
        .collect();

    let guard_name_str = guard_name.to_string();

    // --- Guard struct definition ---
    let guard_struct = quote! {
        #[doc = #guard_doc]
        #[must_use = "guard releases all locks when dropped"]
        #vis struct #guard_name<'a, #impl_prefix #(#generic_names),*> #where_clause {
            #[doc(hidden)]
            lock: &'a #lock_name #ty_generics,
            #(#guard_fields)*
        }

        impl<'a, #impl_prefix #(#generic_names),*> std::fmt::Debug for #guard_name<'a, #bare_prefix #(#generic_names),*> #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(#guard_name_str).finish_non_exhaustive()
            }
        }
    };

    // --- Upgrade/downgrade impl blocks per field (locked fields only) ---
    let mut transition_impls = Vec::new();

    for (i, field) in parsed.fields.iter().enumerate() {
        if field.no_lock {
            continue;
        }

        let gi = field_to_generic[i].unwrap();
        let field_name = &field.name;
        let field_name_str = field_name.to_string();
        let upgrade_method = format_ident!("upgrade_{}", field_name);
        let downgrade_method = format_ident!("downgrade_{}", field_name);

        let upgrade_doc = format!(
            "Atomically upgrade `{}` from upgradable read to exclusive write.\n\n\
             Waits for all other readers to drain. Other fields remain locked as before.\n\n\
             # Deadlock warning\n\n\
             While waiting for readers to drain, this guard continues holding all other locks. \
             If another task holds a read lock on `{}` and is waiting to upgrade a different \
             field that *this* guard holds, both tasks will deadlock.\n\n\
             To upgrade multiple fields safely, either acquire them as `write_*()` upfront \
             or use [`.relock()`](Self::relock) to drop all locks and re-acquire with the \
             desired modes.",
            field_name_str, field_name_str
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
            .filter(|(j, _)| *j != gi)
            .map(|(_, name)| name)
            .collect();

        let other_fields: Vec<proc_macro2::TokenStream> = parsed
            .fields
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| {
                let n = &f.name;
                quote!(#n: self.#n,)
            })
            .collect();

        let make_params = |mode: proc_macro2::TokenStream| -> Vec<proc_macro2::TokenStream> {
            (0..locked_count)
                .map(|j| {
                    if j == gi {
                        mode.clone()
                    } else {
                        let f = &generic_names[j];
                        quote!(#f)
                    }
                })
                .collect()
        };

        let upgrade_input = make_params(quote!(smart_lock::UpgradeLocked));
        let write_output = make_params(quote!(smart_lock::WriteLocked));
        let read_output = make_params(quote!(smart_lock::ReadLocked));
        let write_input = make_params(quote!(smart_lock::WriteLocked));

        let try_upgrade_method = format_ident!("try_upgrade_{}", field_name);
        let try_upgrade_doc = format!(
            "Try to upgrade `{}` from upgradable read to exclusive write without blocking.\n\n\
             Returns `Ok` with the upgraded guard on success, or `Err` with the original \
             guard unchanged if other readers are active.\n\n\
             Unlike `.upgrade_{}().await`, this never blocks and cannot deadlock.",
            field_name_str, field_name_str
        );

        // Upgrade from UpgradeLocked + Downgrade from UpgradeLocked + Try upgrade
        transition_impls.push(quote! {
            impl<'a, #impl_prefix #(#free_generics),*> #guard_name<'a, #bare_prefix #(#upgrade_input),*> #where_clause {
                #[doc = #upgrade_doc]
                #vis async fn #upgrade_method(self) -> #guard_name<'a, #bare_prefix #(#write_output),*> {
                    #guard_name {
                        lock: self.lock,
                        #field_name: self.#field_name.upgrade().await,
                        #(#other_fields)*
                    }
                }

                #[doc = #try_upgrade_doc]
                #vis fn #try_upgrade_method(self) -> Result<#guard_name<'a, #bare_prefix #(#write_output),*>, Self> {
                    match self.#field_name.try_upgrade() {
                        Ok(upgraded) => Ok(#guard_name {
                            lock: self.lock,
                            #field_name: upgraded,
                            #(#other_fields)*
                        }),
                        Err(original) => Err(#guard_name {
                            lock: self.lock,
                            #field_name: original,
                            #(#other_fields)*
                        }),
                    }
                }

                #[doc = #downgrade_from_upgrade_doc]
                #vis fn #downgrade_method(self) -> #guard_name<'a, #bare_prefix #(#read_output),*> {
                    #guard_name {
                        lock: self.lock,
                        #field_name: self.#field_name.downgrade(),
                        #(#other_fields)*
                    }
                }
            }
        });

        // Downgrade from WriteLocked
        transition_impls.push(quote! {
            impl<'a, #impl_prefix #(#free_generics),*> #guard_name<'a, #bare_prefix #(#write_input),*> #where_clause {
                #[doc = #downgrade_from_write_doc]
                #vis fn #downgrade_method(self) -> #guard_name<'a, #bare_prefix #(#read_output),*> {
                    #guard_name {
                        lock: self.lock,
                        #field_name: self.#field_name.downgrade(),
                        #(#other_fields)*
                    }
                }
            }
        });
    }

    // --- relock() method ---
    let lock_bounds: Vec<proc_macro2::TokenStream> = generic_names
        .iter()
        .map(|f| quote!(#f: smart_lock::LockMode))
        .collect();

    let relock_impl = quote! {
        impl<'a, #impl_prefix #(#lock_bounds),*> #guard_name<'a, #bare_prefix #(#generic_names),*> #where_clause {
            /// Drop all held locks and return a fresh builder for the same lock.
            ///
            /// This lets you re-acquire a different set of fields without dropping
            /// the lock reference.
            ///
            /// **Warning:** There is a moment between dropping the old locks and
            /// acquiring new ones where no locks are held. Other tasks may modify
            /// fields during this gap. Do not assume atomicity across a `relock()`.
            #vis fn relock(self) -> #builder_name<'a, #bare_prefix #(#all_unlocked),*> {
                #builder_name { lock: self.lock, _marker: std::marker::PhantomData }
            }
        }
    };

    quote! {
        #guard_struct
        #(#transition_impls)*
        #relock_impl
    }
}
