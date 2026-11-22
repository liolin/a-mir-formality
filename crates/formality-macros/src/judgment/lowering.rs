use proc_macro2::Span;
use quote::quote_spanned;
use syn::{Expr, Ident, Token, Type};

use crate::judgment::{
    ast::{Ast, Comprehesion, ForAll, Premise, PremiseKind},
    internal_ident,
};

pub fn lower(mut judgment: Ast) -> syn::Result<Ast> {
    judgment.body.rules.rules = judgment
        .body
        .rules
        .rules
        .into_iter()
        .map(|mut rule| {
            rule.premises.premises = rule
                .premises
                .premises
                .into_iter()
                .fold(Vec::new(), lower_premise)
                .into();
            rule
        })
        .collect();

    Ok(judgment)
}

fn lower_premise(mut acc: Vec<Premise>, premise: Premise) -> Vec<Premise> {
    match premise.kind {
        PremiseKind::Comprehension(comprehesion) => {
            acc.append(&mut lower_comprehension(comprehesion, premise.span))
        }
        PremiseKind::ForAll(for_all) => {
            acc.push(lower_for_all(for_all, premise.span, premise.commit_point))
        }
        _ => acc.push(premise),
    };
    acc
}

fn lower_comprehension(comprehension: Comprehesion, span: Span) -> Vec<Premise> {
    let identifier = comprehension
        .binding
        .identifier()
        .expect("should alway be a valid identifier");
    let ty = comprehension.binding.ty();
    let value = &comprehension.body.result;

    let def_collected = comprehension_def_collect(span);

    let collect = comprehension_collect(value, span);
    let for_all = lower_for_all(
        comprehension_into_for_all(comprehension, collect),
        span,
        None,
    );

    let conclusion = comprehension_conclusion(identifier, ty, span);

    vec![def_collected, for_all, conclusion]
}

fn comprehension_def_collect(span: Span) -> Premise {
    let collected = internal_ident("collected");
    let collected = syn::parse2(quote_spanned! (span=> let #collected: Vec<_> = Vec::new()))
        .expect("is always valid");

    Premise {
        kind: PremiseKind::Let(collected),
        span,
        commit_point: None,
    }
}

fn comprehension_collect(value: &Expr, span: Span) -> Premise {
    let collected = internal_ident("collected");
    let next = internal_ident("next");

    let collect = syn::parse2(quote_spanned! {span=>
        let #collected = {
            let mut #next = #collected.clone();
            #next.push(::formality_core::Upcast::upcast(&#value));
            #next
        }
    })
    .expect("is always valid");

    Premise {
        kind: PremiseKind::Let(collect),
        span,
        commit_point: None,
    }
}

fn comprehension_conclusion(identifier: Ident, ty: Type, span: Span) -> Premise {
    let collected = internal_ident("collected");
    let result = internal_ident("result");

    let conclusion = syn::parse2(quote_spanned! {span=>
    let #identifier = {
        let #result: #ty = #collected.iter().cloned().collect();
        #result
    }})
    .expect("is always valid");

    Premise {
        kind: PremiseKind::Let(conclusion),
        span,
        commit_point: None,
    }
}

fn comprehension_into_for_all(mut comprehension: Comprehesion, collect: Premise) -> ForAll {
    let collected = internal_ident("collected");
    comprehension.body.premises.premises.push_back(collect);

    ForAll {
        identifier: comprehension.loop_var,
        collection: comprehension.collection,
        state: vec![collected],
        body: comprehension.body.premises,
    }
}

fn lower_for_all(mut for_all: ForAll, span: Span, commit_point: Option<Token![!]>) -> Premise {
    for_all.body.premises = for_all
        .body
        .premises
        .into_iter()
        .fold(Vec::new(), lower_premise)
        .into();

    Premise {
        kind: PremiseKind::ForAll(for_all),
        span,
        commit_point,
    }
}
