use std::ops::Deref;

use itertools::Itertools;

use crate::grammar::{
    expr::{
        Block, Expr, ExprData, FieldExpr, Init, Label, LabelId, PlaceExpr, PlaceExprData, Stmt,
    },
    rust_builder::RustBuilder,
    Binder, FieldName, RefKind, Ty, ValueId,
};

impl RustBuilder {
    pub fn build_block(&mut self, block: &Block) -> String {
        // let label = block.label.map(|l| l.id.deref());
        let statements = block
            .stmts
            .iter()
            .map(|stmt| self.build_stmt(stmt))
            .collect::<Vec<_>>()
            .join("\n");

        format!("{{ {statements} }}")
    }

    pub fn build_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Let {
                label,
                id,
                ty,
                init,
            } => self.build_let(label.as_ref(), id, ty, init.as_ref()),
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => self.build_if(condition, then_block, else_block),
            Stmt::Expr { expr } => self.build_expr(expr),
            Stmt::Loop { label, body } => self.build_loop(label.as_ref(), body),
            Stmt::Break { label } => self.build_break(label),
            Stmt::Continue { label } => self.build_continue(label),
            Stmt::Return { expr } => self.build_return(expr),
            Stmt::Block(block) => self.build_block(block),
            Stmt::Exists { binder } => self.build_exists(binder),
        }
    }

    pub fn build_let(
        &mut self,
        _label: Option<&Label>,
        id: &ValueId,
        ty: &Ty,
        init: Option<&Init>,
    ) -> String {
        let id = id.deref();
        let ty = self.pretty_print_type(ty).unwrap();
        let init = init
            .map(|i| format!(" = {}", self.build_expr(&i.expr)))
            .unwrap_or_default();

        format!("let mut {id}: {ty}{init};")
    }

    pub fn build_if(&mut self, condition: &Expr, then_block: &Block, else_block: &Block) -> String {
        let condition = self.build_expr(condition);
        let then_block = self.build_block(then_block);
        let else_block = self.build_block(else_block);

        format!("if {condition} {{ {then_block} }} else {{ {else_block} }}")
    }

    pub fn build_loop(&mut self, label: Option<&Label>, body: &Block) -> String {
        let label = label
            .map(|l| format!("{}: ", l.id.deref()))
            .unwrap_or_default();
        let body = self.build_block(body);

        format!("{label}loop {{ {body} }}")
    }

    pub fn build_break(&mut self, label: &LabelId) -> String {
        let label = label.deref();
        format!("break {label};")
    }

    pub fn build_continue(&mut self, label: &LabelId) -> String {
        let label = label.deref();
        format!("continue {label};")
    }

    pub fn build_return(&mut self, expr: &Expr) -> String {
        let expr = self.build_expr(expr);
        format!("return {expr};")
    }

    pub fn build_exists(&mut self, binder: &Binder<Block>) -> String {
        self.with_binder(binder, |term, pp| Ok(pp.build_block(term)))
            .unwrap()
    }

    pub fn build_expr(&mut self, expr: &Expr) -> String {
        match expr.data() {
            ExprData::Assign { place, expr } => format!(
                "{} = {};",
                self.build_place_expr(place),
                self.build_expr(expr)
            ),
            ExprData::Call { callee, args } => format!(
                "{}({})",
                self.build_expr(callee),
                args.iter().map(|arg| self.build_expr(arg)).join(", ")
            ),
            ExprData::Literal { value, ty } => format!("{value}_{}", self.pretty_print_scalar(ty)),
            ExprData::True => format!("true"),
            ExprData::False => format!("false"),
            ExprData::Ref { kind, lt, place } => {
                let kind = if matches!(kind, RefKind::Shared) {
                    ""
                } else {
                    "mut"
                };
                let lt = self.pretty_print_lt(lt).unwrap();
                let place = self.build_place_expr(place);
                format!("&{lt} {kind} {place}")
            }
            ExprData::Place(place_expr) => self.build_place_expr(place_expr),
            ExprData::Turbofish { id, args } => {
                let id = id.deref();
                let args = args
                    .iter()
                    .map(|arg| self.pretty_print_parameter(arg))
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
                    .join(", ");
                format!("{id}::<{args}>")
            }
            ExprData::Struct {
                field_exprs,
                adt_id,
                turbofish,
            } => {
                let adt_id = adt_id.deref();
                let args = turbofish
                    .parameters
                    .iter()
                    .map(|arg| self.pretty_print_parameter(arg))
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
                    .join(", ");
                let turbofish = if args.is_empty() {
                    args
                } else {
                    format!("<{args}>")
                };
                let field_exprs = field_exprs
                    .iter()
                    .map(|f| self.build_field_expr(f))
                    .join(", ");

                format!("{adt_id}{turbofish} {{ {field_exprs} }}")
            }
        }
    }

    pub fn build_place_expr(&mut self, place_expr: &PlaceExpr) -> String {
        match place_expr.data() {
            PlaceExprData::Var(value_id) => format!("{}", value_id.deref()),
            PlaceExprData::Deref { prefix } => format!("*{}", self.build_place_expr(prefix)),
            PlaceExprData::Parens(place_expr) => format!("({})", self.build_place_expr(place_expr)),
            PlaceExprData::Field { prefix, field_name } => format!(
                "{}.{}",
                self.build_place_expr(prefix),
                self.build_field_name(field_name)
            ),
        }
    }

    pub fn build_field_expr(&mut self, field_expr: &FieldExpr) -> String {
        let value = self.build_expr(&field_expr.value);
        match &field_expr.name {
            FieldName::Id(id) => format!("{}: {value}", id.deref()),
            FieldName::Index(_) => format!("{value}"),
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn blub() {
        // This code should not compile with rustc and formality
        crate::assert_rust!(
            [
                crate Foo {
                    fn foo() -> i32 {
                        exists<'r0, 'r1> {
                            let v1: i32 = 0 _ i32;
                            let v2: &mut 'r0 i32 = &mut 'r1 v1;
                            // This should result in an error
                            v1 = 1 _ i32;
                            return *v2;
                        }
                    }
                }
            ],
            fn foo() -> i32 {
                {
                    let mut v1: i32 = 0_i32;
                    let mut v2 = &mut v1;
                    v1 = 1_i32; // <-- ERROR
                    return *v2;
                }
            }
        )
    }
}
