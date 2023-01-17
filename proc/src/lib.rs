#![crate_type = "proc-macro"]
extern crate proc_macro;

use proc_macro_crate::{crate_name, FoundCrate};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens, quote_spanned};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Token, spanned::Spanned,
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

mod kw {
    syn::custom_keyword!(F);
    syn::custom_keyword!(S);
    syn::custom_keyword!(before);
    syn::custom_keyword!(after);
}

#[derive(PartialEq, Eq)]
enum SourceKind {
    Future,
    Stream,
}

struct SourcedBranch {
    kind: SourceKind,
    source: Expr,
    item: Ident,
    body: Expr,
}

struct HookBranch {
    kind: HookKind,
    body: Expr,
}

#[derive(PartialEq, Eq)]
enum HookKind {
    Before,
    After,
}

enum Branch {
    Sourced(SourcedBranch),
    Hook(HookBranch),
}

impl Parse for Branch {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let lookahead = input.lookahead1();
        let branch_type = if lookahead.peek(kw::F) {
            input.parse::<kw::F>()?;
            SourceKind::Future
        } else if lookahead.peek(kw::S) {
            input.parse::<kw::S>()?;
            SourceKind::Stream
        } else if lookahead.peek(Token![@]) {
            input.parse::<Token![@]>()?;

            let lookahead = input.lookahead1();

            let kind = if lookahead.peek(kw::before) {
                input.parse::<kw::before>()?;
                HookKind::Before
            } else if lookahead.peek(kw::after) {
                input.parse::<kw::after>()?;
                HookKind::After
            } else {
                return Err(lookahead.error());
            };

            input.parse::<Token![=>]>()?;

            let body: Expr = input.parse()?;

            return Ok(Self::Hook(HookBranch { kind, body }));

        } else {
            let lookahead_error = lookahead.error();
            let error = syn::parse::Error::new(
                lookahead_error.span(),
                "Expected `F`, `S`, `@before` or `@after`.\
                `F` must prefix a Future, while `S` must prefix a Stream."
            );

            return Err(error)
        };

        let stream: Expr = input.parse()?;
        input.parse::<Token![=>]>()?;

        input.parse::<Token![|]>()?;
        let item: Ident = input.parse()?;
        input.parse::<Token![|]>()?;

        let body: Expr = input.parse()?;

        Ok(Self::Sourced(SourcedBranch { item, source: stream, body, kind: branch_type }))

    }
}

struct Select {
    branches: Punctuated<Branch, Token![,]>,
}

impl Parse for Select {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        Ok(Self {
            branches: input.parse_terminated(Branch::parse)?,
        })
    }
}

impl Select {
    fn split(self) -> SelectSplit {
        let mut split = SelectSplit { before: vec![], after: vec![], sourced: vec![] };

        for branch in self.branches.into_iter() {
            match branch {
                Branch::Hook(h) =>{
                    if h.kind == HookKind::Before {
                        split.before.push(h)
                    } else {
                        split.after.push(h)
                    }
                },
                Branch::Sourced(s) => split.sourced.push(s),
            }
        }

        split
    }
}

struct SelectSplit {
    before: Vec<HookBranch>,
    after: Vec<HookBranch>,
    sourced: Vec<SourcedBranch>
}

#[proc_macro]
pub fn select_loop(input: TokenStream) -> TokenStream {
    let crate_import = import();
    let select = parse_macro_input!(input as Select);
    let select = select.split();

    let before = select.before.iter().map(|branch| &branch.body);
    let after = select.after.iter().map(|branch| &branch.body);

    let branch_variant_type: Vec<_> = select
        .sourced
        .iter()
        .enumerate()
        .map(|(i, _branch)| Ident::new(&format!("T{i}"), Span::call_site()))
        .collect();

    let branch_variant_name: Vec<_> = select
        .sourced
        .iter()
        .enumerate()
        .map(|(i, _branch)| Ident::new(&format!("M{i}"), Span::call_site()))
        .collect();

    let branch_variant_item: Vec<_> = select
        .sourced
        .iter()
        .map(|branch| &branch.item)

        .collect();

    let branch_body = select
        .sourced
        .iter()
        .map(|branch| &branch.body);

    let branch_spawn_source_task =  select.sourced.iter().enumerate().map(|(i, branch)| {
        let convertion = if branch.kind == SourceKind::Future {
            Some(quote_spanned!(branch.source.span()=>
                let source = _crate::__private::futures::FutureExt::into_stream(source);
            ))
        } else {
            None
        };

        let branch_source = &branch.source;
        let branch_variant_name = Ident::new(&format!("M{i}"), Span::call_site());

        quote_spanned!(branch.source.span()=>
            let _abort_on_drop = {
                let mut sender = sender.clone();
                let source = #branch_source;

                #convertion

                let mut stream = source;

                _crate::__private::AbortOnDrop::new(async move {
                    use _crate::__private::futures::{StreamExt, SinkExt};

                    while let Some(item) = stream.next().await {
                        if sender.send(__Message::#branch_variant_name(item)).await.is_err() {
                            return;
                        }
                    }
                })
            };
        )
    });

    quote!({
        #crate_import

        enum __Message<#(#branch_variant_type),*> {
            #( #branch_variant_name (#branch_variant_type) ),*
        }

        let (sender, mut receiver) = _crate::__private::futures::channel::mpsc::channel(0);

        #(#branch_spawn_source_task)*

        loop {
            #({#before})*

            let Some(message) = _crate::__private::futures::StreamExt::next(&mut receiver).await else {
                break;
            };

            match message {
                #(
                    __Message::#branch_variant_name(#branch_variant_item) => #branch_body
                ),*
            }

            #({#after})*
        }
    }).into()
}