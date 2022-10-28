// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use proc_macro2::Ident;
use syn::{Attribute, Data, DataEnum, DataStruct, DeriveInput, Lit, LitStr, Meta, NestedMeta, Path};

#[proc_macro_derive(Serializable, attributes(materialize))]
pub fn derive_serializable(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_serializable_struct(&input, strukt),
        DeriveInput { data: Data::Enum(enoom), .. } => derive_serializable_enum(&input, enoom),
        _ => panic!(),
    }
}

#[proc_macro_derive(Serialize, attributes(materialize))]
pub fn derive_serialize(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_serialize_struct(&input, strukt),
        DeriveInput { data: Data::Enum(enoom), .. } => derive_serialize_enum(&input, enoom),
        _ => panic!(),
    }
}

#[proc_macro_derive(Deserialize, attributes(materialize))]
pub fn derive_deserialize(tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(tokens as DeriveInput);
    match &input {
        DeriveInput { data: Data::Struct(strukt), .. } => derive_deserialize_struct(&input, strukt),
        DeriveInput { data: Data::Enum(enoom), .. } => derive_deserialize_enum(&input, enoom),
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
                #(#field_primitives,)*
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
        None => {
            let i = proc_macro2::Literal::usize_unsuffixed(i);
            quote::quote!(let _serializer = _serializer.serialize_field(&self.#i)?;)
        }
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
            quote::quote!(let (#ident, _strukt) = _strukt.advance().and_then(|(f, s)| Ok((<#ty as #crate_path::Deserialize<'de>>::deserialize(f, _capabilities)?, s)))?;)
        }
        None => {
            let ident = quote::format_ident!("_{}", i);
            let ty = &field.ty;
            quote::quote!(let (#ident, _strukt) = _strukt.advance().and_then(|(f, s)| Ok((<#ty as #crate_path::Deserialize<'de>>::deserialize(f, _capabilities)?, s)))?;)
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
        true => quote::quote!(Self(#(#field_names,)*)),
        false => quote::quote!(Self { #(#field_names),* }),
    };

    proc_macro::TokenStream::from(quote::quote! {
        impl<'de> #crate_path::Deserialize<'de> for #struct_name {
            fn deserialize(_strukt: <Self as #crate_path::Serializable>::Primitive<'de>, _capabilities: &[#crate_path::CapabilityWithDescription]) -> Result<Self, #crate_path::DeserializeError> {
                #(#field_deserializes)*
                Ok(#struct_construction)
            }
        }
    })
}

fn derive_serializable_enum(input: &DeriveInput, _: &DataEnum) -> proc_macro::TokenStream {
    let repr = repr(&input.attrs);
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

    proc_macro::TokenStream::from(quote::quote! {
        impl #crate_path::Serializable for #struct_name {
            type Primitive<'a> = #crate_path::primitives::Enum<'a, #repr>;
        }
    })
}

fn derive_serialize_enum(input: &DeriveInput, enoom: &DataEnum) -> proc_macro::TokenStream {
    let attrs = filter_attrs(&input.attrs).collect::<Vec<_>>();
    let repr = repr(&input.attrs);
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

    let first_discriminant =
        enoom.variants.iter().enumerate().find_map(|(i, variant)| Some((i, &variant.discriminant.as_ref()?.1)));
    let variant_arm_serializes = enoom.variants.iter().enumerate().map(|(i, variant)| {
        let variant_ident = &variant.ident;
        let mut arm = quote::quote!(Self::#variant_ident);
        let mut struct_like = true;
        let mut fields = None;
        for (fi, field) in variant.fields.iter().enumerate() {
            let fields = fields.get_or_insert_with(Vec::new);
            match &field.ident {
                Some(ident) => {
                    fields.push(quote::quote!(#ident));
                }
                None => {
                    struct_like = false;
                    let fi = proc_macro2::Literal::usize_unsuffixed(fi);
                    let ident = quote::format_ident!("_{fi}");
                    fields.push(quote::quote!(#ident));
                }
            }
        }

        let discriminant_value = match &variant.discriminant {
            Some((_, discriminant)) => quote::quote!(#discriminant as #repr),
            None => first_discriminant
                .map(|(di, val)| match i > di {
                    true => {
                        let offset = di + i;
                        let offset = proc_macro2::Literal::usize_unsuffixed(offset);
                        quote::quote!(#val as #repr + #offset as #repr)
                    }
                    false => {
                        let i = proc_macro2::Literal::usize_unsuffixed(i);
                        quote::quote!(#i as #repr)
                    }
                })
                .unwrap_or_else(|| {
                    let i = proc_macro2::Literal::usize_unsuffixed(i);
                    quote::quote!(#i as #repr)
                }),
        };

        match fields {
            Some(fields) => {
                arm.extend(match struct_like {
                    true => quote::quote!({ #(#fields),* }),
                    false => quote::quote!((#(#fields),*)),
                });

                arm.extend(
                    quote::quote!( => _serializer.serialize_variant(&(#discriminant_value), &(#(&#fields,)*))?,),
                );
            }
            None => arm.extend(quote::quote!( => _serializer.serialize_variant(&(#discriminant_value), &())?,)),
        }

        arm
    });

    proc_macro::TokenStream::from(quote::quote! {
        impl #crate_path::Serialize for #struct_name {
            fn serialize<'a>(
                &self,
                _serializer: <Self::Primitive<'a> as #crate_path::serialize::serializers::PrimitiveSerializer<'a>>::Serializer,
            ) -> Result<(), #crate_path::SerializeError> {
                match self {
                    #(#variant_arm_serializes)*
                }
                Ok(())
            }
        }
    })
}

fn derive_deserialize_enum(input: &DeriveInput, enoom: &DataEnum) -> proc_macro::TokenStream {
    let attrs = filter_attrs(&input.attrs).collect::<Vec<_>>();
    let repr = repr(&input.attrs);
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

    let first_discriminant =
        enoom.variants.iter().enumerate().find_map(|(i, variant)| Some((i, &variant.discriminant.as_ref()?.1)));
    let mut consts = quote::quote!();
    let variant_arm_deserializes = enoom
        .variants
        .iter()
        .enumerate()
        .map(|(i, variant)| {
            let variant_ident = &variant.ident;
            let mut struct_like = true;
            let mut fields = None;
            let const_ident = quote::format_ident!("{}_DISCRIMINANT", variant_ident);
            match &variant.discriminant {
                Some((_, discriminant)) => {
                    consts.extend(quote::quote!(const #const_ident: #repr = #discriminant as #repr;))
                }
                None => first_discriminant
                    .as_ref()
                    .map(|(di, val)| match i > *di {
                        true => {
                            let offset = di + i;
                            let offset = proc_macro2::Literal::usize_unsuffixed(offset);
                            consts.extend(quote::quote!(const #const_ident: #repr = #val + #offset;));
                        }
                        false => {
                            let i = proc_macro2::Literal::usize_unsuffixed(i);
                            consts.extend(quote::quote!(const #const_ident: #repr = #i;))
                        },
                    })
                    .unwrap_or_else(|| {
                        let i = proc_macro2::Literal::usize_unsuffixed(i);
                        consts.extend(quote::quote!(const #const_ident: #repr = #i;))
                    }),
            };

            for (fi, field) in variant.fields.iter().enumerate() {
                let fields = fields.get_or_insert_with(Vec::new);
                let field_ty = &field.ty;
                match &field.ident {
                    Some(ident) => {
                        fields.push((quote::quote!(#ident), quote::quote!(#field_ty)));
                    }
                    None => {
                        struct_like = false;
                        let ident = quote::format_ident!("_{fi}");
                        fields.push((quote::quote!(#ident), quote::quote!(#field_ty)));
                    }
                }
            }

            let mut block = quote::quote!();
            match fields {
                Some(fields) => {
                    let (names, types): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
                    block.extend(quote::quote!(let _strukt = _enum.associated_data::<<(#(#types,)*) as Serializable>::Primitive<'de>>()?;));
                    block.extend(quote::quote!(#(let (#names, _strukt) = _strukt.advance().and_then(|(p, s)| Ok((<#types as Deserialize<'de>>::deserialize(p, _capabilities)?, s)))?;)*));
                    block.extend(match struct_like {
                        true => quote::quote!(Ok(Self::#variant_ident { #(#names),* })),
                        false => quote::quote!(Ok(Self::#variant_ident(#(#names),*))),
                    });
                }
                None => block.extend(quote::quote!(Ok(Self::#variant_ident))),
            }

            quote::quote!(#const_ident => { #block })
        })
        .collect::<Vec<_>>();

    proc_macro::TokenStream::from(quote::quote! {
        impl<'de> #crate_path::Deserialize<'de> for #struct_name {
            #[inline]
            #[allow(non_upper_case_globals)]
            fn deserialize(_enum: <Self as #crate_path::Serializable>::Primitive<'de>, _capabilities: &[#crate_path::CapabilityWithDescription]) -> Result<Self, #crate_path::DeserializeError> {
                #consts
                match _enum.discriminant()? {
                    #(#variant_arm_deserializes)*
                    _ => Err(#crate_path::DeserializeError::UnknownDiscriminantValue)
                }
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

fn repr(attrs: &[Attribute]) -> proc_macro2::TokenStream {
    attrs
        .iter()
        .find_map(|attr| {
            let meta = attr.parse_meta().ok()?;
            let Meta::List(list) = meta else { return None };
            if list.path.get_ident()? != "repr" {
                return None;
            }

            match list.nested.first()? {
                NestedMeta::Meta(Meta::Path(path)) => Some(quote::quote!(#path)),
                _ => None,
            }
        })
        .unwrap_or_else(|| quote::quote!(isize))
}
