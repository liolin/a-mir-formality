use std::fmt::{self, Write};

use formality_rust::grammar::{Crate, CrateItem, Fn, Program, Ty, TyData};

// look at: https://doc.rust-lang.org/nightly/nightly-rustc/src/rustc_public/mir/pretty.rs.html

/// Emit the code for the `program`.
pub fn dump_program(program: &Program) -> anyhow::Result<String> {
    let mut buffer = String::new();

    writeln!(&mut buffer, "[")?;
    for c in program.crates.iter() {
        dump_crate(&mut buffer, &c)?;
    }
    write!(&mut buffer, "]")?;
    Ok(buffer)
}

fn dump_crate<W: Write>(w: &mut W, krate: &Crate) -> fmt::Result {
    writeln!(w, "crate {} {{", *krate.id)?;

    let first_error = krate
        .items
        .iter()
        .map(|item| dump_crate_item(w, item))
        .find(Result::is_err);

    if let Some(err) = first_error {
        return err;
    }

    writeln!(w, "}}")
}

fn dump_crate_item<W: Write>(w: &mut W, crate_item: &CrateItem) -> fmt::Result {
    match crate_item {
        CrateItem::Fn(fn_data) => dump_fn(w, fn_data),
        CrateItem::FeatureGate(_feature_gate) => todo!(),
        CrateItem::Struct(_) => todo!(),
        CrateItem::Enum(_) => todo!(),
        CrateItem::Trait(_) => todo!(),
        CrateItem::TraitImpl(_trait_impl) => todo!(),
        CrateItem::NegTraitImpl(_neg_trait_impl) => todo!(),
        CrateItem::Test(_test) => todo!(),
    }
}

fn dump_fn<W: Write>(w: &mut W, fn_data: &Fn) -> fmt::Result {
    let (_, term) = fn_data.binder.open();

    write!(w, "fn {} (", *fn_data.id)?;
    let mut sep = "";
    for local in term.input_tys {
        write!(w, "{}{}", sep, dump_ty_data(&local))?;
        sep = ", ";
    }
    // TODO: Where clauses
    write!(w, ") -> {}", dump_ty_data(&term.output_ty))
}

fn dump_ty_data(ty: &Ty) -> String {
    match ty.data() {
        TyData::RigidTy(rigid_ty) => rigid_ty.name.to_string(),
        TyData::AliasTy(_) => todo!(),
        TyData::PredicateTy(_) => todo!(),
        TyData::Variable(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use formality_rust::grammar::{
        Binder, CrateId, FnBoundData, FnId, MaybeFnBody, Parameter, RigidName,
    };

    use super::*;
    #[test]
    fn test_empty_crate() {
        let mut buffer = String::new();
        let krate = Crate::new(CrateId::new("main"), Vec::<CrateItem>::new());

        dump_crate(&mut buffer, &krate).unwrap();
        assert_eq!(buffer, "crate main {\n}\n".to_string());
    }

    #[test]
    fn test_simple_fn() {
        let mut buffer = String::new();
        let item = CrateItem::Fn(Fn {
            id: FnId::new("run"),
            binder: Binder::dummy(FnBoundData {
                input_tys: vec![],
                output_ty: Ty::rigid(RigidName::Tuple(0), Vec::<Parameter>::new()),
                where_clauses: vec![],
                body: MaybeFnBody::NoFnBody,
            }),
        });
        let krate = Crate::new(CrateId::new("main"), vec![item]);

        dump_crate(&mut buffer, &krate).unwrap();
    }
}
