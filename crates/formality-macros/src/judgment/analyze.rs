use syn::spanned::Spanned;

use crate::judgment::ast::Ast;

pub fn analyze(ast: Ast) -> syn::Result<Ast> {
    for rule in ast.rules() {
        if rule.conclusion.patterns.len() < ast.signature.arguments.len() {
            return Err(syn::Error::new(
                rule.conclusion.patterns.span(),
                "fewer patterns in rule than arguments on fn",
            ));
        }

        if rule.conclusion.patterns.len() > ast.signature.arguments.len() {
            return Err(syn::Error::new(
                rule.conclusion.patterns.span(),
                "more patterns in rule than arguments on fn",
            ));
        }

        if rule.conclusion.name != *ast.name() {
            return Err(syn::Error::new(
                rule.conclusion.patterns.span(),
                format!(
                    "Wrong name in the conclusion: Got {}, expected {}",
                    rule.conclusion.name,
                    ast.name()
                ),
            ));
        }
    }

    Ok(ast)
}
