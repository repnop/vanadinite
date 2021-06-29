// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    Expr, Ident, ItemFn, LitStr, Token,
};

fn to_color_code<'a, 'b>(name: &'a str, clear: &'b str) -> &'b str {
    match name.trim_start_matches("__") {
        "clear" => clear,
        "fullclear" => "\x1B[0m",
        "black" => "\x1B[30m",
        "red" => "\x1B[31m",
        "green" => "\x1B[32m",
        "yellow" => "\x1B[33m",
        "blue" => "\x1B[34m",
        "magenta" => "\x1B[35m",
        "cyan" => "\x1B[36m",
        "white" => "\x1B[37m",
        "brightblack" => "\x1B[90m",
        "brightred" => "\x1B[91m",
        "brightgreen" => "\x1B[92m",
        "brightyellow" => "\x1B[93m",
        "brightblue" => "\x1B[94m",
        "brightnagenta" => "\x1B[95m",
        "brightcyan" => "\x1B[96m",
        "brightwhite" => "\x1B[97m",
        _ => panic!("unknown color code: {}", name),
    }
}

struct ColoredPrint {
    output_str: LitStr,
    used_colors: HashSet<Ident>,
    args: Vec<Expr>,
    clear_color: Ident,
}

impl ColoredPrint {
    fn process_input_str(whole_color_name: &Option<Ident>, input_str: &LitStr) -> (String, HashSet<Ident>) {
        let value = input_str.value();
        let mut output_str = String::with_capacity(value.len());
        let mut used_colors = HashSet::new();

        if let Some(s) = whole_color_name {
            output_str.push_str("{__");
            output_str.push_str(&s.to_string());
            output_str.push('}');

            used_colors.insert(Ident::new(&format!("__{}", s), input_str.span()));
        }

        let mut chars = value.chars();
        let mut just_processed_color = false;

        while let Some(c) = chars.next() {
            match c {
                '{' => {
                    output_str.push('{');
                    match chars.next() {
                        Some('{') => output_str.push_str("{{"),
                        Some('}') => {
                            output_str.push('}');
                        }
                        Some(':') => {
                            output_str.push(':');
                            #[allow(clippy::while_let_on_iterator)]
                            while let Some(c) = chars.next() {
                                match c {
                                    '}' => {
                                        output_str.push('}');
                                        break;
                                    }
                                    c => output_str.push(c),
                                }
                            }
                        }
                        Some('#') => {
                            output_str.pop();
                            let mut ident = String::new();

                            #[allow(clippy::while_let_on_iterator)]
                            while let Some(c) = chars.next() {
                                match c {
                                    ';' => {
                                        output_str.push_str("{__");
                                        output_str.push_str(&ident);
                                        output_str.push('}');

                                        used_colors.insert(Ident::new(&format!("__{}", ident), input_str.span()));

                                        let mut n_brace = 0;
                                        while let Some(c) = chars.next() {
                                            match c {
                                                '{' => {
                                                    output_str.push_str("{{");
                                                    n_brace += 1;
                                                }
                                                '}' if n_brace == 0 => break,
                                                '}' => {
                                                    output_str.push_str("}}");
                                                    n_brace -= 1;
                                                }
                                                c => output_str.push(c),
                                            }
                                        }

                                        output_str.push_str("{__clear}");
                                        used_colors.insert(Ident::new("__clear", input_str.span()));
                                        break;
                                    }
                                    '\'' => {
                                        output_str.push_str("{__");
                                        output_str.push_str(&ident);
                                        output_str.push('}');

                                        used_colors.insert(Ident::new(&format!("__{}", ident), input_str.span()));

                                        let mut n_brace = 0;
                                        while let Some(c) = chars.next() {
                                            match c {
                                                '{' => {
                                                    output_str.push('{');
                                                    n_brace += 1;
                                                }
                                                '}' if n_brace == 0 => break,
                                                '}' => {
                                                    output_str.push('}');
                                                    n_brace -= 1;
                                                }
                                                c => output_str.push(c),
                                            }
                                        }

                                        output_str.push_str("{__clear}");
                                        used_colors.insert(Ident::new("__clear", input_str.span()));
                                        break;
                                    }
                                    ' ' => {
                                        output_str.push_str("{__");
                                        output_str.push_str(&ident);
                                        output_str.push('}');
                                        used_colors.insert(Ident::new(&format!("__{}", ident), input_str.span()));

                                        output_str.push('{');
                                        just_processed_color = true;
                                        break;
                                    }
                                    ':' => {
                                        output_str.push_str("{__");
                                        output_str.push_str(&ident);
                                        output_str.push('}');
                                        used_colors.insert(Ident::new(&format!("__{}", ident), input_str.span()));

                                        output_str.push('{');
                                        output_str.push(c);
                                        just_processed_color = true;
                                        break;
                                    }
                                    '}' => {
                                        output_str.push_str("{__");
                                        output_str.push_str(&ident);
                                        output_str.push('}');
                                        used_colors.insert(Ident::new(&format!("__{}", ident), input_str.span()));

                                        output_str.push_str("{}");
                                        output_str.push_str("{__clear}");
                                        used_colors.insert(Ident::new("__clear", input_str.span()));
                                        break;
                                    }
                                    c => ident.push(c),
                                }
                            }
                        }
                        Some(c) => output_str.push(c),
                        None => {
                            output_str.push('{');
                            break;
                        }
                    }
                }
                '}' if just_processed_color => {
                    just_processed_color = false;
                    output_str.push('}');
                    output_str.push_str("{__clear}");
                    used_colors.insert(Ident::new("__clear", input_str.span()));
                }
                c => output_str.push(c),
            }
        }

        if whole_color_name.is_some() {
            output_str.push_str("{__fullclear}");
            used_colors.insert(Ident::new("__fullclear", input_str.span()));
        }

        (output_str, used_colors)
    }
}

impl Parse for ColoredPrint {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (whole_line_color_code, whole_line_color_name, input_str) = match (input.peek(Ident), input.peek(LitStr)) {
            (true, false) => {
                let name = input.parse::<Ident>()?;
                let code = to_color_code(&name.to_string(), "");
                input.parse::<Token![,]>()?;

                (Some(code), Some(name), input.parse::<LitStr>()?)
            }
            (false, true) => (None, None, input.parse::<LitStr>()?),
            _ => return Err(input.lookahead1().error()),
        };

        if !input.peek(Token![,]) && whole_line_color_code.is_none() {
            return Ok(Self {
                output_str: input_str,
                used_colors: HashSet::new(),
                args: Vec::new(),
                clear_color: syn::parse_quote!(__fullclear),
            });
        } else if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }

        let args = input.parse_terminated::<_, Token![,]>(Expr::parse)?.into_iter().collect();

        let (output, used_colors) = Self::process_input_str(&whole_line_color_name, &input_str);
        let output_str = LitStr::new(&output, input_str.span());

        Ok(Self {
            output_str,
            used_colors,
            args,
            clear_color: whole_line_color_name.unwrap_or_else(|| syn::parse_quote!(__fullclear)),
        })
    }
}

#[proc_macro]
pub fn colored_print(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(print!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn colored_println(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(println!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn info(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(log::info!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn debug(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(log::debug!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn trace(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(log::trace!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn warn(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(log::warn!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro]
pub fn error(input: TokenStream) -> TokenStream {
    let ColoredPrint { clear_color, output_str, used_colors, args } = syn::parse_macro_input!(input as ColoredPrint);
    let clear_color = to_color_code(&clear_color.to_string(), "");
    let named_colors = used_colors
        .into_iter()
        .map(|c| {
            let color_str = LitStr::new(to_color_code(&c.to_string(), clear_color), output_str.span());
            quote!(#c = crate::io::terminal::ColorEscape(#color_str))
        })
        .collect::<Vec<_>>();

    let extra_comma = if !named_colors.is_empty() && !args.is_empty() { quote!(,) } else { quote!() };

    TokenStream::from(quote!(log::error!(#output_str, #(#args),* #extra_comma #(#named_colors),*)))
}

#[proc_macro_attribute]
pub fn test(_: TokenStream, input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;

    TokenStream::from(quote! {
        #[test_case]
        fn #name() {
            crate::print!(
                "test {} ... ",
                concat!(module_path!(), "::", stringify!(#name)).trim_start_matches("vanadinite::")
            );
            #body
            crate::println!("{}ok{}", crate::io::terminal::GREEN, crate::io::terminal::CLEAR);
        }
    })
}
