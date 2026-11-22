use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, Attribute, BinOp, Expr, ExprBinary, ExprUnary, Ident, Pat, UnOp};

use crate::judgment::{
    ast::{
        self, Argument, Ast, Binding, Collect, Fail, ForAll, If, IfLet, Implication, In, Judgment,
        Label, Let, Premise, PremiseKind, Premises, Rule, Scope, Signature,
    },
    internal_ident,
};

pub type Rust = TokenStream;

#[derive(Debug, Clone)]
pub(crate) struct State<'a> {
    pub span: Span,
    pub name: &'a str,
    pub proof_trees: &'a Ident,
    pub rest: Premises,
}

pub fn emit(judgment: Ast) -> Rust {
    let attributes = emit_attributes(&judgment.attributes);
    let signature_tokens = emit_signature(&judgment.signature);
    let judgment_struct = emit_judgment_struct(&judgment);
    let function_body = emit_function_body(&judgment);

    quote! {
        #attributes
        #signature_tokens {
            #judgment_struct

            #function_body
        }
    }
}

fn emit_attributes(attributes: &Vec<Attribute>) -> TokenStream {
    quote! {
        #(#attributes)*
    }
}

fn emit_signature(
    Signature {
        visibility,
        name,
        arguments,
        return_,
    }: &Signature,
) -> TokenStream {
    let args: Vec<_> = arguments
        .iter()
        .map(|Argument { name, ty }| quote! { #name: impl ::formality_core::Upcast<#ty> })
        .collect();

    quote! {
        #visibility fn #name(#(#args),*) -> ::formality_core::ProvenSet<#return_>
    }
}

fn emit_judgment_struct(judgment: &Judgment) -> TokenStream {
    let input_tys: Vec<_> = judgment.input_tys();
    let input_names = judgment.input_names();

    let name = judgment.name();
    let debug_input_names = judgment.debug_input_names();

    quote! {
        #[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Clone)]
        struct __JudgmentStruct(#(#input_tys),*);

        ::formality_core::cast_impl!(__JudgmentStruct);

        impl std::fmt::Debug for __JudgmentStruct {
            fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut f = fmt.debug_struct(stringify!(#name));
                let __JudgmentStruct(#(#input_names),*) = self;
                #(
                    f.field(stringify!(#debug_input_names), #debug_input_names);
                )*
                f.finish()
            }
        }
    }
}

fn emit_function_body(judgment: &Judgment) -> TokenStream {
    let input_name: Vec<_> = judgment.input_names();
    let input_ty: Vec<_> = judgment.input_tys();
    let output_ty = judgment.output();
    let name = judgment.name();
    let debug_input_name = judgment.debug_input_names();

    let input = internal_ident("input");
    let input_string = internal_ident("input_string");
    let output = internal_ident("output");
    let failed_rules = internal_ident("failed_rules");
    let cache = internal_ident("R");

    let assertions = emit_assertions(judgment);
    let trivials = emit_trivials(judgment);
    let rules = emit_rules(judgment);

    quote! {
        #(let #input_name: #input_ty = ::formality_core::Upcast::upcast(#input_name);)*

        #assertions
        #trivials

        let mut #failed_rules = ::formality_core::set![];
        let #input = __JudgmentStruct(#(#input_name),*);
        let #output = ::formality_core::judgment::execute_judgment::<
            __JudgmentStruct,
            #output_ty,
        >(
            // Tracing span:
            |input| {
                let __JudgmentStruct(#(#input_name),*) = input;
                tracing::debug_span!(
                    stringify!(#name),
                    #(?#debug_input_name),*
                )
            },

            // Per-judgmnet cache:
            {
                thread_local! {
                    static #cache: ::formality_core::judgment::JudgmentCache<__JudgmentStruct, #output_ty> = Default::default()
                }
                &#cache
            },

            // Input:
            #input.clone(),

            // Execute rules:
            |input: __JudgmentStruct| {
                let mut #output: ::formality_core::Map<#output_ty, ::formality_core::judgment::ProofTree> = ::formality_core::Map::new();

                #failed_rules.clear();

                #[allow(unused)]
                let #input_string = format!("{:?}", input);
                #rules
                #output
            },
        );

        if !#output.is_empty() {
            ::formality_core::ProvenSet::proven(#output)
        } else {
            ::formality_core::ProvenSet::failed_rules(
                &#input,
                ::formality_core::judgment::FailureLocation::caller(),
                #failed_rules,
            )
        }
    }
}

fn emit_assertions(judgment: &Judgment) -> TokenStream {
    let input_name: Vec<_> = judgment.input_names();
    let assert_expr: Vec<_> = judgment.assertions();

    let assertions = assert_expr
        .iter()
        .fold(TokenStream::new(), |mut stream, expr| {
            let span = expr.span();
            stream.extend(quote_spanned!(span=> ::formality_core::judgment::JudgmentAssertion::assert(#expr, stringify!(#expr));));
            stream
        });

    quote! {
        {
            #(
                #[allow(unused_variables)]
                let #input_name = &#input_name;
            )*
            #assertions
        }
    }
}

fn emit_trivials(judgment: &Judgment) -> TokenStream {
    judgment
        .body
        .trivials
        .trivials
        .iter()
        .fold(TokenStream::new(), |mut stream, trivial| {
            let span = trivial.span;
            let expr = &trivial.hypothesis;
            let result = &trivial.conclusion;
            stream.extend(quote_spanned!{span=>
                                         // Trivial cases are an (important) optimization that lets
                                         // you cut out all the normal rules.
                                         if #expr {
                                             let trivial_result = #result;
                                             let (file, line, column) = (file!(), line!(), column!());
                                             let proof_tree = ::formality_core::judgment::ProofTree::with_all(
                                                 format!("trivial, as {} is true: {trivial_result:?}", stringify!(#expr)),
                                                 Default::default(),
                                                 None,
                                                 file,
                                                 line,
                                                 column,
                                                 Default::default(),
                                             );
                                             return ::formality_core::ProvenSet::singleton((trivial_result, proof_tree));
                                         }
            });
            stream
        })
}

fn emit_rules(judgment: &Judgment) -> TokenStream {
    let rules: Vec<_> = judgment
        .body
        .rules
        .rules
        .iter()
        .map(|rule| emit_rule(judgment, rule))
        .collect();

    quote! {
        #(#rules)*
    }
}

fn emit_rule(judgment: &Judgment, rule: &Rule) -> TokenStream {
    let input_names = judgment.input_names();
    let judgment_name = judgment.name();
    let conclusion_name = rule.conclusion.name.clone();
    let rule_name = rule.label.name.clone();
    let match_default = !rule.has_commit_point();

    let child_proof_trees = internal_ident("child_proof_trees");

    let mut premises = rule.premises.clone();
    premises.premises.push_back(Premise {
        kind: PremiseKind::Collect(Collect::Judgment {
            judgment_name: judgment.name().clone(),
            result_value: rule.conclusion.result.clone(),
            label: rule.label.clone(),
        }),
        span: Span::mixed_site(),
        commit_point: None,
    });

    let premises = emit_premises(premises, &rule.label.name.value(), &child_proof_trees);

    // TODO: Maybe move to its own function
    let rule_stream = quote! {
        tracing::trace_span!("matched rule", rule = #rule_name, judgment = stringify!(#judgment_name)).in_scope(|| {
            tracing::debug!("matched rule {:?}", #rule_name);
            #[allow(unused_mut)]
            let mut #child_proof_trees: Vec<::formality_core::judgment::ProofTree> = Vec::new();

            #premises
        });
    };
    let match_and_rule = emit_match(judgment, rule, rule_stream);

    let span = conclusion_name.span();
    let wrong_name_in_conclusion = quote_spanned! {span=>
                               // give the user a type error if the name they gave
                               // in the conclusion is not the same as the name of the
                               // function
                               #[allow(dead_code)]
                               struct WrongJudgmentNameInConclusion;
                               const _: WrongJudgmentNameInConclusion = {
                                   let #judgment_name = WrongJudgmentNameInConclusion;
                                   #conclusion_name
                               };
    };

    quote! {
        {
            #wrong_name_in_conclusion
            if let Some(__JudgmentStruct(#(#input_names),*)) = Some(&input) {
                let __judgment_name = stringify!(#judgment_name);
                let __attributes: Vec<(String, String)> = vec![
                    #((stringify!(#input_names).to_string(), format!("{:?}", #input_names))),*
                ];
                let mut __matched = #match_default;
                #match_and_rule
            }
        };
    }
}

fn emit_match(judgment: &Judgment, rule: &Rule, rule_stream: TokenStream) -> TokenStream {
    rule.conclusion
        .patterns
        .iter()
        .zip(&judgment.signature.arguments)
        .rev()
        .fold(
            rule_stream,
            |acc, (pattern, Argument { name, ty })| match pattern {
                Pat::Ident(pat_ident) => {
                    quote! { { let #pat_ident = #name; #acc}; }
                }
                Pat::Lit(_) | Pat::TupleStruct(_) | Pat::Struct(_) | Pat::Path(_) => {
                    quote! {
                        if let Some(#pattern) = &::formality_core::Downcast::downcast(#name) {
                            #acc
                        };
                    }
                }
                pat => todo!("emit match not implemented for {pat:#?}"),
            },
        )
}

fn emit_premises(mut premises: Premises, name: &str, proof_trees: &Ident) -> TokenStream {
    let Some(premise) = premises.premises.pop_front() else {
        return TokenStream::new();
    };

    let span = premise.span;
    let state = State {
        span,
        name,
        proof_trees,
        rest: premises,
    };

    match &premise.kind {
        PremiseKind::IfLet(if_let) => emit_if_let(if_let, state),
        PremiseKind::If(if_) => emit_if(if_, state),
        PremiseKind::ForAll(for_all) => emit_for_all(for_all, state),
        PremiseKind::Fail(fail) => emit_fail(fail, state),
        PremiseKind::Implication(implication) => emit_implication(implication, state),
        PremiseKind::Let(let_) => emit_let(let_, state),
        PremiseKind::Comprehension(_comprehension) => {
            unreachable!("Comprehension is consumed during the lowering phase")
        }
        PremiseKind::In(in_) => emit_in(in_, state),
        PremiseKind::Scope(scope) => emit_scope(scope, state),
        PremiseKind::Collect(collect) => emit_collect(collect, state),
    }
}

pub(crate) fn emit_if_let(
    if_let: &IfLet,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let expr = &if_let.expression;
    let p = &if_let.pattern;

    let value = internal_ident("value");
    let err = internal_ident("e");
    let cause = internal_ident("cause");

    let body = emit_premises(rest, name, proof_trees);
    let record_failure = emit_record_failure(name, span);

    quote_spanned! {span=>
        match ::formality_core::judgment::try_catch(|| Ok(#expr)) {
            Ok(#value) => {
                if let #p = &#value {
                    #body
                } else {
                    let #cause = ::formality_core::judgment::RuleFailureCause::IfLetDidNotMatch {
                        pattern: stringify!(#p).to_string(),
                        value: format!("{:?}", #value),
                    };
                    #record_failure
                }
            }

            Err(#err) => {
                let #cause = #err;
                #record_failure
            }
        }
    }
}

pub(crate) fn emit_if(
    if_: &If,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let origcond = &if_.expression;
    let record_failure = emit_record_failure(name, span);
    let (argn, arge, cond) = if_helper(&if_.expression);

    let if_then = internal_ident("if_then");
    let cause = internal_ident("cause");

    let body = emit_premises(rest, name, proof_trees);

    // TODO: Get the arguments for better error reporting
    quote_spanned! {span=>
        #(
            let #argn = #arge;
        )*
        let #if_then = ::formality_core::judgment::IfThen::new(
            stringify!(#origcond),
            vec![
                #(
                    (
                        stringify!(#arge),
                        ::formality_core::judgment::DebugString::new(#argn),
                    ),
                )*
            ],
        );
        #[allow(unused_braces)]
        if {
            #cond
        } {
            #proof_trees.push(::formality_core::judgment::ProofTree::leaf(format!("{:?}", #if_then)));
            #body
        } else {
            let #cause = ::formality_core::judgment::RuleFailureCause::IfFalse(#if_then);
            #record_failure
        };
    }
}

// TODO: Extend for more cases.
// Currently mirrors the bahvior of the existing macro_rule macro.
// This functions is also ugly as hell.
fn if_helper(expression: &Expr) -> (Vec<Ident>, Vec<&Expr>, TokenStream) {
    let mut argn = vec![];
    let mut arge = vec![];
    let mut cond = quote! { #expression };

    let arg0n = Ident::new("arg0", Span::mixed_site());
    let arg1n = Ident::new("arg1", Span::mixed_site());
    let arg2n = Ident::new("arg2", Span::mixed_site());

    match &expression {
        Expr::Call(expr_call) => {
            if expr_call.args.len() == 1 {
                let arg0e = &expr_call.args[0];
                arge.push(arg0e);
                argn.push(arg0n);
                cond = quote! { arg0(arg1) };
            } else if expr_call.args.len() == 2 {
                let arg0e = &expr_call.args[0];
                arge.push(arg0e);
                argn.push(arg0n);
                cond = quote! { arg0(arg1, arg2) };
            }
        }
        Expr::Binary(ExprBinary {
            attrs,
            left,
            op,
            right,
        }) => match (&op, &left.as_ref(), &right.as_ref()) {
            (BinOp::Eq(_), Expr::Path(_), Expr::Path(_)) => {
                let arg0 = left;
                let arg1 = right;
                arge.push(arg0);
                arge.push(arg1);
                argn.push(arg0n);
                argn.push(arg1n);
                cond = quote! { arg0 == arg1 }
            }
            (BinOp::Ne(_), Expr::Path(_), Expr::Path(_)) => {
                let arg0 = left;
                let arg1 = right;
                arge.push(arg0);
                arge.push(arg1);
                argn.push(arg0n);
                argn.push(arg1n);
                cond = quote! { arg0 != arg1 }
            }
            _ => {}
        },
        Expr::Unary(ExprUnary {
            attrs,
            op: UnOp::Not(_),
            expr,
        }) => {
            let (argnx, argex, condx) = if_helper(expr);
            argn = argnx;
            arge = argex;
            cond = quote! { !#cond };
        }
        _ => {}
    }

    (argn, arge, cond)
}

pub(crate) fn emit_for_all(
    for_all: &ForAll,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let collection = &for_all.collection;
    let loop_var = &for_all.identifier;
    let with_var = &for_all.state;

    let next_carried_and_proof = internal_ident("next_carried_and_proof");
    let collection_var = internal_ident("collection");
    let alternatives = internal_ident("alternatives");
    let next_alternatives = internal_ident("next_alternatives");
    let attributes = internal_ident("attributes");
    let item_proof_trees = internal_ident("item_proof_trees");
    let file = internal_ident("file");
    let line = internal_ident("line");
    let column = internal_ident("column");
    let loop_carried = internal_ident("loop_carried");
    let for_all_proof_trees = internal_ident("for_all_proof_trees");
    let next_carried = internal_ident("next_carried");
    let next_proof = internal_ident("next_proof");
    let proofs = internal_ident("proofs");
    let len = internal_ident("len");

    let collection_span = collection.span();
    let debug_information = quote_spanned!(collection_span=> (file!(), line!(), column!()));

    // let collect = quote! { #next_carried_and_proof.push(((#(#with_var.clone(),)*), #item_proof_trees.clone())); };
    // TODO: I do not like, that I modify the AST in codegen.
    // Is there a better way?
    let mut for_all_body = for_all.body.clone();
    for_all_body.premises.push_back(Premise {
        kind: PremiseKind::Collect(Collect::Loop {
            next: next_carried_and_proof.clone(),
            with_var: with_var.clone(),
            item_proof_trees: item_proof_trees.clone(),
        }),
        span,
        commit_point: None,
    });
    let inner = emit_premises(for_all_body, name, &item_proof_trees);
    let body = emit_premises(rest, name, &proof_trees);
    let record_failure = emit_record_failure(name, span);

    quote_spanned! {span=>
        {
            let #collection_var = #collection;
            let (#file, #line, #column) = #debug_information;

            let mut #alternatives = ::formality_core::Map::new();
            ::formality_core::judgment::insert_smallest_proof(
                &mut #alternatives,
                (#(#with_var.clone(),)*),
                ::formality_core::judgment::ProofTree::new("for_all", None, vec![]),
            );
            for #loop_var in #collection_var {
                let mut #next_alternatives = ::formality_core::Map::new();
                for (#loop_carried, #for_all_proof_trees) in #alternatives {
                    let #loop_var = Clone::clone(&#loop_var);
                    #[allow(unused)]
                    let (#(#with_var,)*) = &#loop_carried;

                    let #attributes = vec![("item".to_string(), format!("{:?}", #loop_var))];
                    // I shadown the outer `child_proof_trees` delibertly, such that the generated code for the premises still uses the correct vector.
                    let mut #item_proof_trees = vec![];
                    let mut #next_carried_and_proof = vec![];
                    #inner

                    for (#next_carried, #next_proof) in #next_carried_and_proof {
                        let mut #proofs = #for_all_proof_trees.children.clone();
                        #proofs.push(::formality_core::judgment::ProofTree::with_all(
                            "for_all",
                            #attributes.clone(),
                            None,
                            &#file[..],
                            #line,
                            #column,
                            #next_proof,
                        ));
                        ::formality_core::judgment::insert_smallest_proof(
                            &mut #next_alternatives,
                            #next_carried,
                            ::formality_core::judgment::ProofTree::new("for_all", None, #proofs),
                        );
                    }
                }
                #alternatives = #next_alternatives;
                if #alternatives.is_empty() {
                    break;
                }
            }

            for (#loop_carried, #for_all_proof_trees) in #alternatives {
                #[allow(unused)]
                let (#(#with_var,)*) = &#loop_carried;

                let #len = #proof_trees.len();
                #proof_trees.push(#for_all_proof_trees);
                #body
                #proof_trees.truncate(#len);
            }
        };
    }
}

pub(crate) fn emit_fail(
    fail: &Fail,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let fmt = &fail.message;
    let arg = &fail.expressions;

    let message = internal_ident("message");
    let cause = internal_ident("cause");

    let record_failure = emit_record_failure(name, span);
    quote! {
        {
            #[allow(unused_variables)]
            let _ = &#proof_trees;
            let #message = format!(#fmt #(, #arg)*);
            #[allow(unused_assignments)]
            { __matched = true; }
            let #cause = ::formality_core::judgment::RuleFailureCause::ExplicitFailure { message: #message };
            #record_failure
        }
    }
}

pub(crate) fn emit_let(
    let_: &Let,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let expression = &let_.expression;
    let t = &let_.binding.ty();
    let p = match &let_.binding {
        Binding::Pattern { pattern } => quote! { #pattern },
        Binding::Identifier { identifier, ty } => quote! { #identifier},
    };
    let proof_tree = internal_ident("proof_tree");
    let p_var = internal_ident("p");
    let e = internal_ident("e");
    let cause = internal_ident("cause");

    let body = emit_premises(rest, name, proof_trees);
    let record_failure = emit_record_failure(name, span);

    quote_spanned! {span=>
        match ::formality_core::judgment::try_catch::<#t>(|| Ok(#expression)) {
            Ok(#p_var) => {
                let #proof_tree = ::formality_core::judgment::ProofTree::leaf(format!("{} = {:?}", stringify!(#p), #p_var));
                let #p = &#p_var;
                #proof_trees.push(#proof_tree);
                #body
            }

            Err(#e) => {
                let #cause = #e;
                #record_failure
            }
        };
    }
}

pub(crate) fn emit_in(
    in_: &In,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let expression = &in_.expression;
    let pattern = &in_.pattern;
    let e = internal_ident("e");
    let value = internal_ident("value");
    let proof_tree = internal_ident("proof_tree");
    let len = internal_ident("len");
    let cause = internal_ident("cause");

    let body = emit_premises(rest, name, proof_trees);
    let record_failure = emit_record_failure(name, span);

    quote_spanned! {span=>
        if let Err(#e) = ::formality_core::judgment::member_of(
            #expression,
            || stringify!(#expression).to_string(),
            |(#value, #proof_tree)| {
                let #pattern = &#value;

                // Remember the size of the child proof tree stack
                let #len = #proof_trees.len();

                // Push this child proof tree
                #proof_trees.push(#proof_tree);

                #body

                // Restore the original child proof tree stack; note that there may be multiple entries
                // in case "non-iterative" steps like `(let ...)` and `(if ...)` are interspersed.
                assert!(#proof_trees.len() > #len);
                #proof_trees.truncate(#len);
            },
        ) {
            let #cause = #e;
            #record_failure
        }
    }
}

pub(crate) fn emit_implication(implication: &Implication, state: State) -> TokenStream {
    if ast::is_unit_pattern(&implication.conclusion) {
        emit_implication_unit(implication, state)
    } else {
        emit_implication_general(implication, state)
    }
}

fn emit_implication_unit(
    implication: &Implication,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let i = &implication.hypothesis;
    let proof_tree = internal_ident("proof_tree");
    let e = internal_ident("e");
    let cause = internal_ident("cause");

    let record_failure = emit_record_failure(name, span);
    let body = emit_premises(rest, name, proof_trees);

    quote_spanned! {span=>
        match ::formality_core::judgment::CheckProven::check_proven(
            #i,
        ) {
            Ok(#proof_tree) => {
                #proof_trees.push(#proof_tree);
                #body
            }
            Err(#e) => {
                let #cause = #e;
                #record_failure
            }
        }
    }
}

fn emit_implication_general(
    implication: &Implication,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let i = &implication.hypothesis;
    let p = &implication.conclusion;
    let e = internal_ident("e");
    let value = internal_ident("value");
    let proof_tree = internal_ident("proof_tree");
    let len = internal_ident("len");
    let cause = internal_ident("cause");

    let record_failure = emit_record_failure(name, span);
    let body = emit_premises(rest, name, proof_trees);
    quote_spanned! {span=>
        if let Err(#e) = ::formality_core::judgment::EachProof::each_proof(
            #i,
            |(#value, #proof_tree)| {
                #[allow(irrefutable_let_patterns)]
                if let #p = &#value {
                    // Remember the size of the child proof tree stack
                    let #len = #proof_trees.len();

                    // Push this child proof tree
                    #proof_trees.push(#proof_tree);

                    #body

                    // Restore the original child proof tree stack; note that there may be multiple entries
                    // in case "non-iterative" steps like `(let ...)` and `(if ...)` are interspersed.
                    assert!(#proof_trees.len() > #len);
                    #proof_trees.truncate(#len);
                } else {
                    let #cause = ::formality_core::judgment::RuleFailureCause::IfLetDidNotMatch {
                        pattern: stringify!(#p).to_string(),
                        value: format!("{:?}", #value),
                    };
                    #record_failure
                }
            },
        ) {
            let #cause = #e;
            #record_failure
        }

    }
}

pub(crate) fn emit_scope(
    scope: &Scope,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let expression = &scope.expression;
    let inside = &scope.inside;
    let with_var = &scope.state;
    let e = internal_ident("e");
    let cause = internal_ident("cause");
    let scope_var = internal_ident("scope");
    let scope_proofs = internal_ident("scope_proofs");
    let values = internal_ident("values");
    let inner_proof_trees = internal_ident("inner_proof_trees");
    let leave_proof_tree = internal_ident("leave_proof_tree");
    let len = internal_ident("len");
    let scope_proof_trees = internal_ident("scope_proof_trees");
    let inside_var = internal_ident("inside");

    let record_failure = emit_record_failure(name, span);

    // let collect =
    //     quote! { #scope_proofs.push(((#(#with_var.clone(),)*), #inner_proof_trees.clone())); };

    let mut scope_body = scope.body.clone();
    scope_body.premises.push_back(Premise {
        kind: PremiseKind::Collect(Collect::Scope {
            scope_proofs: scope_proofs.clone(),
            with_var: with_var.clone(),
            inner_proof_trees: inner_proof_trees.clone(),
        }),
        span,
        commit_point: None,
    });
    let inner = emit_premises(scope_body, name, &inner_proof_trees);
    let body = emit_premises(rest, name, proof_trees);
    quote_spanned! {span=>
        match ::formality_core::judgment::try_catch(|| Ok(#expression)) {
            Err(#e) => {
                let #cause = #e;
                #record_failure
            }

            Ok((#scope_var, #inside_var)) => {
                // Prove the nested conditions. They see what is inside the scope
                // (and, being in a block, what they bind ends with it). Collect
                // each proof's values for the `with` variables.
                let mut #scope_proofs = vec![];
                {
                    #[allow(unused_variables)]
                    let #inside = &#inside_var;
                    #[allow(unused_mut)]
                    let mut #inner_proof_trees: Vec<::formality_core::judgment::ProofTree> = Vec::new();
                    #inner
                }

                // Leave the scope with each, and continue with what comes out.
                for (#values, #inner_proof_trees) in #scope_proofs {
                    if let Err(#e) = ::formality_core::judgment::EachProof::each_proof(
                        ::formality_core::judgment::Scope::leave(&#scope_var, #values),
                        |(#values, #leave_proof_tree)| {
                            #[allow(unused_variables)]
                            let (#(#with_var,)*) = &#values;

                            let #len = #proof_trees.len();

                            let mut #scope_proof_trees = #inner_proof_trees.clone();
                            #scope_proof_trees.push(#leave_proof_tree);
                            #proof_trees.push(::formality_core::judgment::ProofTree::new(
                                "scope",
                                None,
                                #scope_proof_trees,
                            ));

                            #body

                            assert!(#proof_trees.len() > #len);
                            #proof_trees.truncate(#len);
                        },
                    ) {
                        let #cause = #e;
                        #record_failure
                    }
                }
            }
        };
    }
}

fn emit_record_failure(rule_name: &str, span: Span) -> TokenStream {
    // failed_rules, __matched, cause are in scope
    let file = internal_ident("file");
    let line = internal_ident("line");
    let column = internal_ident("column");
    let cause = internal_ident("cause");
    let failed_rules = internal_ident("failed_rules");

    quote_spanned! {span=>
        let #file = file!();
        let #line = line!();
        let #column = column!();

        if __matched {
            tracing::debug!(
                "rule {rn} failed because {cause} ({file}:{line}:{column})",
                rn = #rule_name,
                cause = #cause,
                file = #file,
                line = #line,
                column = #column,
            );
            #failed_rules.insert(
                ::formality_core::judgment::FailedRule {
                    rule_name: Some(#rule_name.to_string()),
                    file: #file.to_string(),
                    line: #line,
                    column: #column,
                    cause: #cause,
                }
            );
        } else {
            tracing::trace!(
                "rule {rn} failed because {cause} ({file}:{line}:{column})",
                rn = #rule_name,
                cause = #cause,
                file = #file,
                line = #line,
                column = #column,
            );
        };
    }
}

fn emit_collect(collect: &Collect, state: State) -> TokenStream {
    let next_carried_and_proof = internal_ident("next_carried_and_proof");
    match collect {
        Collect::Judgment {
            judgment_name,
            result_value,
            label,
        } => emit_collect2(judgment_name, result_value, label, state),
        Collect::Scope {
            scope_proofs,
            with_var,
            inner_proof_trees,
        } => {
            let span = state.span;
            quote_spanned! (span=> #scope_proofs.push(((#(#with_var.clone(),)*), #inner_proof_trees.clone())); )
        }
        Collect::Loop {
            next,
            with_var,
            item_proof_trees,
        } => {
            let span = state.span;
            quote_spanned! (span=> #next.push(((#(#with_var.clone(),)*), #item_proof_trees.clone())); )
        }
    }
}

fn emit_collect2(
    judgment_name: &Ident,
    result_value: &Expr,
    label: &Label,
    State {
        span,
        name,
        proof_trees,
        rest,
    }: State,
) -> TokenStream {
    let rule_name = &label.name;
    let span = label.name.span();

    let output = internal_ident("output");
    let child_proof_trees = internal_ident("child_proof_trees");
    let file = internal_ident("file");
    let line = internal_ident("line");
    let column = internal_ident("column");
    let attributes = internal_ident("attributes");
    let proof_tree = internal_ident("proof_tree");
    let result = internal_ident("result");

    let location =
        quote_spanned! (span=> let (#file, #line, #column) = (file!(), line!(), column!()););

    quote! {
        {
            let #result = ::formality_core::Upcast::upcast(#result_value);
            let mut #attributes = __attributes.clone();
            #attributes.push(("result".to_string(), format!("{:?}", #result)));
            #location
            let #proof_tree = ::formality_core::judgment::ProofTree::with_all(
                __judgment_name,
                #attributes,
                Some(#rule_name),
                #file,
                #line,
                #column,
                #child_proof_trees.clone(),
            );
            tracing::debug!("produced {:?} from rule {:?} in judgment {:?}", #result, #rule_name, stringify!(#judgment_name));
            ::formality_core::judgment::insert_smallest_proof(&mut #output, #result, #proof_tree);
        };
    }
}
