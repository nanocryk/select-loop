#![crate_type = "proc-macro"]
extern crate proc_macro;

use proc_macro_crate::{crate_name, FoundCrate};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Token,
};

fn import() -> impl ToTokens {
    let Ok(found_crate) = crate_name("select-loop") else {
        panic!("Cannot find dependency 'select-loop' in Cargo.toml")
    };

    match found_crate {
        FoundCrate::Itself => quote!( use select_loop as _crate; ),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( use #ident as _crate; )
        }
    }
}

struct SelectBranch {
    stream: Expr,
    item: Ident,
    body: Expr,
}

impl Parse for SelectBranch {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let stream: Expr = input.parse()?;
        input.parse::<Token![=>]>()?;

        input.parse::<Token![|]>()?;
        let item: Ident = input.parse()?;
        input.parse::<Token![|]>()?;

        let body: Expr = input.parse()?;

        Ok(Self { item, stream, body })
    }
}

struct Select {
    branches: Punctuated<SelectBranch, Token![,]>,
}

impl Parse for Select {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        Ok(Self {
            branches: input.parse_terminated(SelectBranch::parse)?,
        })
    }
}

#[proc_macro]
pub fn select_loop(input: TokenStream) -> TokenStream {
    let crate_import = import();

    let select = parse_macro_input!(input as Select);

    let branch_stream = select.branches.iter().map(|branch| &branch.stream);

    let branch_variant_type: Vec<_> = select
        .branches
        .iter()
        .enumerate()
        .map(|(i, _branch)| Ident::new(&format!("__Type{i}"), Span::call_site()))
        .collect();

    let branch_variant_name: Vec<_> = select
        .branches
        .iter()
        .enumerate()
        .map(|(i, _branch)| Ident::new(&format!("__Stream{i}"), Span::call_site()))
        .collect();

    let branch_variant_item: Vec<_> = select
        .branches
        .iter()
        .map(|branch| &branch.item)

        .collect();

    let branch_body = select
        .branches
        .iter()
        .map(|branch| &branch.body);

    quote!({
        #crate_import

        enum __Message<#(#branch_variant_type),*> {
            #( #branch_variant_name (#branch_variant_type) ),*
        }

        let (sender, mut receiver) = _crate::__private::futures::channel::mpsc::channel(0);

        #(
            let _abort_on_drop = {
                let mut sender = sender.clone();
                let mut stream = #branch_stream;
                _crate::__private::AbortOnDrop::new(async move {
                    while let Some(item) = _crate::__private::futures::StreamExt::next(&mut stream).await {
                        if _crate::__private::futures::SinkExt::send(&mut sender, __Message::#branch_variant_name(item)).await.is_err() {
                            return;
                        }
                    }
                })
            };
        )*

        while let Some(message) = ::futures::StreamExt::next(&mut receiver).await {
            match message {
                #(
                    __Message::#branch_variant_name(#branch_variant_item) => #branch_body
                ),*
            }
        }
    }).into()
}