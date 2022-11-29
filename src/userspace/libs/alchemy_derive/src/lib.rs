// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use proc_macro2::Span;
use syn::{Attribute, GenericParam, Generics, ItemStruct, Meta, NestedMeta};

#[proc_macro_derive(PackedStruct)]
pub fn derive_packed_struct(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ItemStruct { attrs, ident, mut generics, fields, .. } = syn::parse_macro_input!(input as ItemStruct);
    add_packed_struct_generic_bounds(&mut generics);
    if let Err(e) = check_repr_c_or_transparent(&attrs) {
        return proc_macro::TokenStream::from(e.into_compile_error());
    } else if fields.is_empty() {
        return proc_macro::TokenStream::from(
            syn::Error::new(Span::call_site(), "A `PackedStruct` must contain at least one field").to_compile_error(),
        );
    }

    let field_asserts = fields.into_iter().enumerate().fold(quote::quote!(), |mut tkns, (i, field)| {
        let ty = &field.ty;
        let name = field.ident.map(|i| format!("`{i}`")).unwrap_or_else(|| format!("#{i}"));
        tkns.extend(quote::quote! {
            const _: AssertPackedStruct<#ty> = AssertPackedStruct { _p: core::marker::PhantomData };
            let (size, align) = (core::mem::size_of::<#ty>(), core::mem::align_of::<#ty>());
            if total_size % align != 0 {
                panic!(concat!("Internal padding found before field ", #name));
            }
            total_size += size;
        });
        tkns
    });

    proc_macro::TokenStream::from(quote::quote! {
        unsafe impl alchemy::OnlyValidBitPatterns for #ident #generics {}
        unsafe impl alchemy::PackedStruct for #ident #generics {}

        const _: () = {
            struct AssertPackedStruct<T: alchemy::PackedStruct> { _p: core::marker::PhantomData<T> }

            let mut total_size = 0;
            #field_asserts

            if total_size % core::mem::align_of::<#ident>() != 0 {
                panic!(concat!("struct `", stringify!(#ident), "` contains end padding"));
            }
        };
    })
}

fn add_packed_struct_generic_bounds(generics: &mut Generics) {
    for generic in &mut generics.params {
        if let GenericParam::Type(ty) = generic {
            ty.bounds.push(syn::parse_quote!(alchemy::PackedStruct));
        }
    }
}

fn check_repr_c_or_transparent(attrs: &[Attribute]) -> Result<(), syn::Error> {
    for attr in attrs {
        let Meta::List(meta) = attr.parse_meta()? else { continue };
        let Some(ident) = meta.path.get_ident() else { continue };
        if ident == "repr" {
            if let Some(NestedMeta::Meta(Meta::Path(repr))) = meta.nested.first() {
                let Some(repr) = repr.get_ident() else { continue };

                if repr == "C" || repr == "transparent" {
                    return Ok(());
                }
            }
        }
    }

    Err(syn::Error::new(Span::call_site(), "`PackedStruct`s must have either `#[repr(C)]` or `#[repr(transparent)]`"))
}
