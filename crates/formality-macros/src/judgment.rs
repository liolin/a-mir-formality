#![expect(dead_code)]
#![expect(unused_variables)]

mod analyze;
mod ast;
mod codegen;
mod parse;
#[cfg(test)]
mod tests;

// TODO: Add support for renaming formality-core, by extending the judgment_fn! syntax with the new name of the core package.
// E.g. judgment{my_formality_core, fn ...}
pub fn judgment(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = match parse::parse(input.into()) {
        Ok(ast) => ast,
        Err(err) => return err.into_compile_error().into(),
    };

    let model = match analyze::analyze(ast) {
        Ok(model) => model,
        Err(err) => return err.into_compile_error().into(),
    };

    let tokens = codegen::emit(model);
    proc_macro::TokenStream::from(tokens)
}

mod keyword {
    syn::custom_keyword!(debug);
    syn::custom_keyword!(assert);
    syn::custom_keyword!(trivial);

    syn::custom_keyword!(for_all);
    syn::custom_keyword!(fail);
    syn::custom_keyword!(scope);
    syn::custom_keyword!(with);
}
