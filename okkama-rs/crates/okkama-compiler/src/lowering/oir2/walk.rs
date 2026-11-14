//! Reuse THIR traversal for finite call specialization discovery.
use crate::analysis::thir;

pub(super) fn block(block: &thir::Block, visit: &mut dyn FnMut(&thir::Expr)) {
    thir::each_block_expr(block, &mut |value| expr(value, visit));
}

fn expr(value: &thir::Expr, visit: &mut dyn FnMut(&thir::Expr)) {
    visit(value);
    thir::each_child(value, &mut |child| expr(child, visit));
}
