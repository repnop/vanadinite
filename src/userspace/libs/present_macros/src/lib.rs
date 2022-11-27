// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn main(_args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ItemFn { attrs, vis: _, sig, block } = parse_macro_input!(input as ItemFn);
    let return_ty = match &sig.output {
        syn::ReturnType::Default => quote::quote!(),
        syn::ReturnType::Type(_, ty) => quote::quote!(-> #ty),
    };

    if sig.asyncness.is_none() || sig.ident != "main" {
        return proc_macro::TokenStream::from(
            syn::Error::new(sig.fn_token.span, "`#[present::main]` must be used on an `async fn main`")
                .to_compile_error(),
        );
    }

    proc_macro::TokenStream::from(quote::quote! {
        #(#attrs)*
        fn main() #return_ty {
            let present = present::Present::new();
            present.block_on(async #block)
        }
    })
}
