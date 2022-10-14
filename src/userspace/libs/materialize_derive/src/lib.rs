// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use proc_macro2::{Ident, Span};
use syn::{Attribute, Data, DataStruct, DeriveInput, Lit, LitStr, Meta, NestedMeta, Path};

#[proc_macro_derive(Serializable, attributes(materialize))]
pub fn derive_serializable(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_serializable_struct(&input, strukt),
        _ => panic!(),
    }
}

#[proc_macro_derive(Serialize, attributes(materialize))]
pub fn derive_serialize(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_serialize_struct(&input, strukt),
        _ => panic!(),
    }
}

#[proc_macro_derive(Deserialize, attributes(materialize))]
pub fn derive_deserialize(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_deserialize_struct(&input, strukt),
        _ => panic!(),
    }
}

fn derive_serializable_struct(input: &DeriveInput, strukt: &DataStruct) -> proc_macro::TokenStream {
    let attrs = filter_attrs(&input.attrs).collect::<Vec<_>>();
    let struct_name = &input.ident;
    let crate_path = match attrs.iter().find(|da| da.ident == "reexport_path") {
        Some(DeriveAttr { value: Some(lit), .. }) => match lit.parse::<Path>() {
            Ok(path) => quote::quote!(#path),
            Err(e) => return e.to_compile_error().into(),
        },
        Some(DeriveAttr { value: None, .. }) => {
            return quote::quote!(compile_error!("`reexport_path` requires a valid path value")).into()
        }
        None => quote::quote!(materialize),
    };

    let field_primitives = strukt
        .fields
        .iter()
        .map(|field| {
            let ty = &field.ty;
            quote::quote!(<#ty as #crate_path::Serializable>::Primitive<'a>)
        })
        .collect::<Vec<_>>();

    proc_macro::TokenStream::from(quote::quote! {
        impl #crate_path::Serializable for #struct_name {
            type Primitive<'a> = #crate_path::primitives::Struct<'a, (
                #(#field_primitives),*
            )>;
        }
    })
}

fn derive_serialize_struct(input: &DeriveInput, strukt: &DataStruct) -> proc_macro::TokenStream {
    let attrs = filter_attrs(&input.attrs).collect::<Vec<_>>();
    let struct_name = &input.ident;
    let crate_path = match attrs.iter().find(|da| da.ident == "reexport_path") {
        Some(DeriveAttr { value: Some(lit), .. }) => match lit.parse::<Path>() {
            Ok(path) => quote::quote!(#path),
            Err(e) => return e.to_compile_error().into(),
        },
        Some(DeriveAttr { value: None, .. }) => {
            return quote::quote!(compile_error!("`reexport_path` requires a valid path value")).into()
        }
        None => quote::quote!(materialize),
    };

    let field_serializes = strukt.fields.iter().enumerate().map(|(i, field)| match &field.ident {
        Some(ident) => quote::quote!(let _serializer = _serializer.serialize_field(&self.#ident)?;),
        None => quote::quote!(let _serializer = _serializer.serialize_field(&self.#i)?;),
    });

    proc_macro::TokenStream::from(quote::quote! {
        impl #crate_path::Serialize for #struct_name {
            fn serialize<'a>(
                &self,
                _serializer: <Self::Primitive<'a> as #crate_path::serialize::serializers::PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), #crate_path::SerializeError> {
                #(#field_serializes)*
                Ok(())
            }
        }
    })
}

fn derive_deserialize_struct(input: &DeriveInput, strukt: &DataStruct) -> proc_macro::TokenStream {
    let attrs = filter_attrs(&input.attrs).collect::<Vec<_>>();
    let struct_name = &input.ident;
    let crate_path = match attrs.iter().find(|da| da.ident == "reexport_path") {
        Some(DeriveAttr { value: Some(lit), .. }) => match lit.parse::<Path>() {
            Ok(path) => quote::quote!(#path),
            Err(e) => return e.to_compile_error().into(),
        },
        Some(DeriveAttr { value: None, .. }) => {
            return quote::quote!(compile_error!("`reexport_path` requires a valid path value")).into()
        }
        None => quote::quote!(materialize),
    };

    let is_tuple = strukt.fields.iter().any(|field| field.ident.is_none());
    let field_deserializes = strukt.fields.iter().enumerate().map(|(i, field)| match &field.ident {
        Some(ident) => {
            let ty = &field.ty;
            quote::quote!(let (#ident, _strukt) = _strukt.advance().and_then(|(f, s)| Ok((<#ty as #crate_path::Deserialize<'de>>::deserialize(f)?, s)))?;)
        }
        None => {
            let ident = quote::format_ident!("_{}", i);
            let ty = &field.ty;
            quote::quote!(let (#ident, _strukt) = _strukt.advance().and_then(|(f, s)| Ok((<#ty as #crate_path::Deserialize<'de>>::deserialize(f)?, s)))?;)
        }
    });

    let field_names = strukt.fields.iter().enumerate().map(|(i, field)| match &field.ident {
        Some(ident) => quote::quote!(#ident),
        None => {
            let ident = quote::format_ident!("_{}", i);
            quote::quote!(#ident)
        }
    });

    let struct_construction = match is_tuple {
        true => quote::quote!(Self(#(#field_names),*)),
        false => quote::quote!(Self { #(#field_names),* }),
    };

    proc_macro::TokenStream::from(quote::quote! {
        impl<'de> #crate_path::Deserialize<'de> for #struct_name {
            fn deserialize(_strukt: <Self as Serializable>::Primitive<'de>) -> Result<Self, #crate_path::DeserializeError> {
                #(#field_deserializes)*
                Ok(#struct_construction)
            }
        }
    })
}

struct DeriveAttr {
    ident: Ident,
    value: Option<LitStr>,
}

fn filter_attrs(attrs: &[Attribute]) -> impl Iterator<Item = DeriveAttr> + '_ {
    attrs
        .iter()
        .filter_map(|attr| attr.parse_meta().ok())
        .filter_map(|meta| match meta {
            Meta::List(l) => match l.path.get_ident()? == "materialize" {
                true => Some(l.nested),
                false => None,
            },
            _ => None,
        })
        .flat_map(|nm| {
            nm.into_iter().filter_map(|meta| match meta {
                NestedMeta::Meta(Meta::NameValue(nv)) => Some(DeriveAttr {
                    ident: nv.path.get_ident()?.clone(),
                    value: match nv.lit {
                        Lit::Str(ls) => Some(ls),
                        _ => None,
                    },
                }),
                _ => None,
            })
        })
}
