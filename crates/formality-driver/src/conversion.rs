use std::fmt::{self, Write};
use std::ops::ControlFlow;

// look at: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_public/mir/pretty.rs.html

/// Emit the code for the local crate.
pub fn compile_local_crate() -> ControlFlow<anyhow::Result<String>> {
    let mut code = String::new();
    let krate = rustc_public::local_crate();
    let items = rustc_public::all_local_items();

    let result = write!(&mut code, "[ ")
        .and_then(|()| compile_crate(&mut code, &krate, items))
        .and_then(|()| write!(&mut code, " ]"));

    if let Err(error) = result {
        return std::ops::ControlFlow::Break(Err(anyhow::anyhow!(error)));
    }

    std::ops::ControlFlow::Break(anyhow::Ok(code))
}

fn compile_crate<W: Write>(
    w: &mut W,
    krate: &rustc_public::Crate,
    items: rustc_public::CrateItems,
) -> fmt::Result {
    write!(w, "crate {} {{ ", krate.name)?;

    let first_error = items
        .into_iter()
        .map(|item| compile_crate_item(w, item))
        .find(Result::is_err);

    if let Some(err) = first_error {
        return err;
    }

    write!(w, "}}")
}

fn compile_crate_item<W: Write>(w: &mut W, crate_item: rustc_public::CrateItem) -> fmt::Result {
    match crate_item.kind() {
        rustc_public::ItemKind::Fn => compile_fn(w, crate_item),
        rustc_public::ItemKind::Static => unimplemented!(),
        rustc_public::ItemKind::Const => unimplemented!(),
        rustc_public::ItemKind::Ctor(_kind) => unimplemented!(),
    }
}

fn compile_fn<W: Write>(w: &mut W, crate_item: rustc_public::CrateItem) -> fmt::Result {
    let fn_name = crate_item.0.name();
    let body = crate_item.expect_body();

    write!(w, "fn {fn_name} (")?;
    let mut sep = "";
    for local in body.arg_locals() {
        write!(w, "{}{}", sep, local.ty)?;
        sep = ", ";
    }
    write!(w, ")")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn compile<T: std::marker::Send>(
        code: &str,
        callback: impl Fn() -> std::ops::ControlFlow<T> + std::marker::Sync,
    ) -> Result<T, ()> {
        let mut path = std::env::temp_dir();
        path.push("main.rs");

        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(code.as_bytes()).unwrap();

        let args = vec![
            "formality_driver".to_string(),
            "--crate-type=lib".to_string(),
            "-Awarnings".to_string(),
            path.to_str().unwrap().to_string(),
        ];

        match rustc_public::run!(&args, callback) {
            Err(rustc_public::CompilerError::Interrupted(data)) => Ok(data),
            _ => Err(()),
        }
    }

    #[test]
    fn test_empty_crate() {
        let code = compile("", || {
            let mut code = String::new();
            let krate = rustc_public::local_crate();
            let items = rustc_public::all_local_items();
            compile_crate(&mut code, &krate, items).unwrap();
            std::ops::ControlFlow::Break(code)
        })
        .unwrap();
        assert_eq!(code, "crate main { }".to_string());
    }
}
