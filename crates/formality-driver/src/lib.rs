#![feature(rustc_private)]
extern crate rustc_public;

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_middle;

use rustc_public::{run, CompilerError};

use formality_rust::check::check_all_crates;
use formality_rust::grammar::Program;
use formality_rust::rust::try_term;

#[cfg(test)]
mod test;

pub mod conversion;

pub fn main(rustc_args: Vec<String>) -> anyhow::Result<()> {
    let result = run!(&rustc_args, conversion::compile_local_crate);
    let code = match result {
        Err(CompilerError::Interrupted(code)) => code?,
        Ok(()) => {
            anyhow::bail!("Something went wrong and code generation was performed!")
        }
        _ => anyhow::bail!("Err"),
    };

    dbg!(&code);
    let program: Program = try_term(&code)?;
    let _proof_tree = check_all_crates(&program)?;
    Ok(())
}
