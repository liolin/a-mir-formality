#![feature(rustc_private)]
extern crate rustc_public;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;

use rustc_public::{run, CompilerError};

use formality_rust::check::check_all_crates;
use formality_rust::grammar::Program;

#[cfg(test)]
mod test;

pub mod dump;
pub mod mir;

pub fn main(rustc_args: Vec<String>) -> anyhow::Result<()> {
    let result = run!(&rustc_args, convert);
    let program = match result {
        Err(CompilerError::Interrupted(code)) => code,
        Ok(()) => {
            anyhow::bail!("Something went wrong and code generation was performed!")
        }
        _ => anyhow::bail!("Err"),
    };

    let code = dump::dump_program(&program)?;
    eprintln!("{}", code);
    let _proof_tree = check_all_crates(&program)?;
    Ok(())
}

pub fn convert() -> std::ops::ControlFlow<Program> {
    let krate = rustc_public::local_crate();
    let items = rustc_public::all_local_items();

    let krate = mir::crate_into_formality(krate, items);
    std::ops::ControlFlow::Break(Program {
        crates: vec![krate],
    })
}
