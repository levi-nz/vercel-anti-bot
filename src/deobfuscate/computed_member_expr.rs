use swc_core::ecma::ast::{Expr, Ident, Lit, MemberExpr, MemberProp};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

/// Replaces computed member properties with identifiers.
pub struct Visitor;

impl VisitMut for Visitor {
    fn visit_mut_member_expr(&mut self, member_expr: &mut MemberExpr) {
        member_expr.visit_mut_children_with(self);

        if let MemberProp::Computed(property) = &member_expr.prop {
            if let Expr::Lit(Lit::Str(s)) = &*property.expr {
                member_expr.prop = MemberProp::Ident(Ident::new(
                    s.value.clone(),
                    property.span
                ));
            }
        }
    }
}
