use formality_rust::grammar::{
    Binder, Crate, CrateId, CrateItem, Fn, FnBoundData, FnId, MaybeFnBody, Parameter, RigidName,
    ScalarId, Ty,
};

pub fn crate_into_formality(
    krate: rustc_public::Crate,
    items: Vec<rustc_public::CrateItem>,
) -> Crate {
    let items = items
        .into_iter()
        .map(|item| crate_item_into_formality(item))
        .collect();

    Crate {
        id: CrateId::new(&krate.name),
        items,
    }
}

pub fn crate_item_into_formality(item: rustc_public::CrateItem) -> CrateItem {
    match item.kind() {
        rustc_public::ItemKind::Fn => CrateItem::Fn(fn_into_formality(item)),
        rustc_public::ItemKind::Static => unimplemented!(),
        rustc_public::ItemKind::Const => unimplemented!(),
        rustc_public::ItemKind::Ctor(_kind) => unimplemented!(),
    }
}

pub fn fn_into_formality(item: rustc_public::CrateItem) -> Fn {
    assert_eq!(item.kind(), rustc_public::ItemKind::Fn);
    let name = item.0.name();
    let body = item.expect_body();
    let input_tys: Vec<_> = body
        .arg_locals()
        .iter()
        .map(|arg| local_decl_into_formality(arg))
        .collect();
    let output_ty = local_decl_into_formality(body.ret_local());
    let where_clauses = vec![];
    let body = basic_blocks_into_formality(body.blocks);

    let bound_data = FnBoundData {
        input_tys,
        output_ty,
        where_clauses,
        body,
    };

    Fn {
        id: FnId::new(&name),
        binder: Binder::dummy(bound_data),
    }
}

fn local_decl_into_formality(local_decl: &rustc_public::mir::LocalDecl) -> Ty {
    match local_decl.ty.kind() {
        rustc_public::ty::TyKind::RigidTy(rigid) => rigid_ty_into_formality(rigid),
        rustc_public::ty::TyKind::Alias(_, _) => unimplemented!(),
        rustc_public::ty::TyKind::Param(_) => unimplemented!(),
        rustc_public::ty::TyKind::Bound(_, _) => unimplemented!(),
    }
}

fn rigid_ty_into_formality(ty: rustc_public::ty::RigidTy) -> Ty {
    match ty {
        rustc_public::ty::RigidTy::Bool => Ty::bool(),
        rustc_public::ty::RigidTy::Char => unimplemented!(),
        rustc_public::ty::RigidTy::Int(t) => int_ty_into_formality(t),
        rustc_public::ty::RigidTy::Uint(t) => uint_ty_into_formality(t),
        rustc_public::ty::RigidTy::Float(_) => unimplemented!(),
        rustc_public::ty::RigidTy::Adt(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Foreign(_) => unimplemented!(),
        rustc_public::ty::RigidTy::Str => unimplemented!(),
        rustc_public::ty::RigidTy::Array(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Pat(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Slice(_) => unimplemented!(),
        rustc_public::ty::RigidTy::RawPtr(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Ref(_, _, _) => unimplemented!(),
        rustc_public::ty::RigidTy::FnDef(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::FnPtr(_) => unimplemented!(),
        rustc_public::ty::RigidTy::Closure(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Coroutine(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::CoroutineClosure(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Dynamic(_, _) => unimplemented!(),
        rustc_public::ty::RigidTy::Never => unimplemented!(),
        rustc_public::ty::RigidTy::Tuple(_) => unimplemented!(),
        rustc_public::ty::RigidTy::CoroutineWitness(_, _) => unimplemented!(),
    }
}

fn int_ty_into_formality(ty: rustc_public::ty::IntTy) -> Ty {
    use rustc_public::ty::IntTy::*;
    let params: Vec<Parameter> = vec![];
    let scalar_id = match ty {
        Isize => ScalarId::Isize,
        I8 => ScalarId::I8,
        I16 => ScalarId::I16,
        I32 => ScalarId::I32,
        I64 => ScalarId::I64,
        I128 => ScalarId::I128,
    };
    Ty::rigid(RigidName::ScalarId(scalar_id), params)
}

fn uint_ty_into_formality(ty: rustc_public::ty::UintTy) -> Ty {
    use rustc_public::ty::UintTy::*;
    let params: Vec<Parameter> = vec![];
    let scalar_id = match ty {
        Usize => ScalarId::Usize,
        U8 => ScalarId::U8,
        U16 => ScalarId::U16,
        U32 => ScalarId::U32,
        U64 => ScalarId::U64,
        U128 => ScalarId::U128,
    };
    Ty::rigid(RigidName::ScalarId(scalar_id), params)
}

fn basic_blocks_into_formality(blocks: Vec<rustc_public::mir::BasicBlock>) -> MaybeFnBody {
    if blocks.len() == 0 {
        return MaybeFnBody::NoFnBody;
    }
    unimplemented!();
}
