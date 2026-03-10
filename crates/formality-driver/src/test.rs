use super::*;

fn lib_rustc_args(file: impl Into<String>) -> Vec<String> {
    vec![
        "formality_driver".into(),
        "--crate-type=lib".into(),
        file.into(),
    ]
}

#[test]
fn empty_crate() {
    let rustc_args = lib_rustc_args("test_files/empty_crate.rs");
    main(rustc_args).unwrap();
}
