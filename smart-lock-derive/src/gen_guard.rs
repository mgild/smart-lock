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
        let upgrade_method = format_ident!("upgrade_{}", field_name);
        let downgrade_method = format_ident!("downgrade_{}", field_name);

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
                    #vis async fn #upgrade_method(self) -> #guard_name<'a, #(#bare),*, #(#write_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.upgrade().await,
                            #(#other_fields)*
                        }
                    }

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
                    #vis async fn #upgrade_method(self) -> #guard_name<'a, #(#write_output),*> {
                        #guard_name {
                            lock: self.lock,
                            #field_name: self.#field_name.upgrade().await,
                            #(#other_fields)*
                        }
                    }

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
            #vis struct #guard_name<'a, #(#impl_params),*, #(#generic_names),*> #where_clause {
                #[doc(hidden)]
                lock: &'a #lock_name #ty_generics,
                #(#guard_fields)*
            }
        }
    } else {
        quote! {
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
