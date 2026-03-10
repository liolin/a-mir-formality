#![feature(rustc_private)]
extern crate rustc_public;

fn main() -> anyhow::Result<()> {
    let rustc_args = std::env::args().collect();
    formality_driver::main(rustc_args)?;
    Ok(())
}
