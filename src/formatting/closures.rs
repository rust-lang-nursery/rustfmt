use rustc_ast::{ast, ptr};
use rustc_span::Span;

use crate::config::{lists::*, IndentStyle, SeparatorTactic};
use crate::formatting::{
    attr::get_attrs_from_stmt,
    comment::{contains_comment, rewrite_missing_comment},
    expr::{
        block_contains_comment, is_simple_block, is_unsafe_block, rewrite_block_with_visitor,
        rewrite_cond,
    },
    items::{span_hi_for_param, span_lo_for_param},
    lists::{definitive_tactic, itemize_list, write_list, ListFormatting, Separator},
    overflow::OverflowableItem,
    rewrite::{Rewrite, RewriteContext},
    shape::Shape,
    source_map::SpanUtils,
    utils::{last_line_width, left_most_sub_expr, stmt_expr, NodeIdExt, StmtsExt},
};

// This module is pretty messy because of the rules around closures and blocks:
// FIXME - the below is probably no longer true in full.
//   * if there is a return type, then there must be braces,
//   * given a closure with braces, whether that is parsed to give an inner block
//     or not depends on if there is a return type and if there are statements
//     in that block,
//   * if the first expression in the body ends with a block (i.e., is a
//     statement without needing a semi-colon), then adding or removing braces
//     can change whether it is treated as an expression or statement.

pub(crate) fn rewrite_closure(
    capture: ast::CaptureBy,
    is_async: &ast::Async,
    movability: ast::Movability,
    fn_decl: &ast::FnDecl,
    body: &ast::Expr,
    span: Span,
    arg_span: Span,
    context: &RewriteContext<'_>,
    shape: Shape,
) -> Option<String> {
    debug!("rewrite_closure {:?}", body);

    let (prefix, extra_offset) = rewrite_closure_fn_decl(
        capture, is_async, movability, fn_decl, body, span, context, shape,
    )?;
    // 1 = space between `|...|` and body.
    let body_shape = shape.offset_left(extra_offset)?;

    if let ast::ExprKind::Block(ref block, _) = body.kind {
        // The body of the closure is an empty block.
        if block.stmts.is_empty() && !block_contains_comment(context, block) {
            return body
                .rewrite(context, shape)
                .map(|s| format!("{} {}", prefix, s));
        }

        // Whether a closure block wrapping may not be preserved (#4394).
        let can_try_rewrite_without_block = if context.inside_macro() {
            false
        } else if context.config.preserve_closure_block_wrapping()
            && context.snippet(body.span).trim_start().starts_with('{')
        {
            false
        } else {
            true
        };

        let result = match fn_decl.output {
            ast::FnRetTy::Default(_) if can_try_rewrite_without_block => {
                try_rewrite_without_block(body, &prefix, capture, context, shape, body_shape)
            }
            _ => None,
        };

        result.or_else(|| {
            // Either we require a block, or tried without and failed.
            rewrite_closure_block(block, &prefix, context, body_shape)
        })
    } else {
        // If there are comments between the fn decl and the body, the body (+ comments) need to be
        // wrapped in a block. Since there's no return type annotation on closures with expr
        // bodies, look for comments after the argument block.
        // Since attrs come before the body, check up to the first attr if there is one.
        let first_span = body
            .attrs
            .first()
            .map(|attr| attr.span)
            .unwrap_or(body.span);
        let between_span = Span::between(arg_span, first_span);
        if contains_comment(context.snippet(between_span)) {
            return rewrite_closure_with_block(body, &prefix, context, body_shape).and_then(|rw| {
                let mut parts = rw.splitn(2, '\n');
                let head = parts.next()?;
                let rest = parts.next()?;
                let block_shape = shape.block_indent(context.config.tab_spaces());
                let indent = block_shape.indent.to_string_with_newline(context.config);
                let missing_comment = rewrite_missing_comment(between_span, block_shape, context)?;
                Some(format!(
                    "{}{}{}{}{}",
                    head,
                    indent,
                    missing_comment,
                    indent,
                    rest.trim()
                ))
            });
        }
        rewrite_closure_expr(body, &prefix, context, body_shape).or_else(|| {
            // The closure originally had a non-block expression, but we can't fit on
            // one line, so we'll insert a block.
            rewrite_closure_with_block(body, &prefix, context, body_shape)
        })
    }
}

fn try_rewrite_without_block(
    expr: &ast::Expr,
    prefix: &str,
    capture: ast::CaptureBy,
    context: &RewriteContext<'_>,
    shape: Shape,
    body_shape: Shape,
) -> Option<String> {
    let expr = get_inner_expr(expr, prefix, context);

    if is_block_closure_forced(context, expr, capture) {
        rewrite_closure_with_block(expr, prefix, context, shape)
    } else {
        rewrite_closure_expr(expr, prefix, context, body_shape)
    }
}

fn get_inner_expr<'a>(
    expr: &'a ast::Expr,
    prefix: &str,
    context: &RewriteContext<'_>,
) -> &'a ast::Expr {
    if let ast::ExprKind::Block(ref block, _) = expr.kind {
        if !needs_block(block, prefix, context) {
            // block.stmts.len() == 1 except with `|| {{}}`;
            // https://github.com/rust-lang/rustfmt/issues/3844
            if let Some(expr) = block.stmts.find_non_empty().and_then(stmt_expr) {
                return get_inner_expr(expr, prefix, context);
            }
        }
    }

    expr
}

// Figure out if a block is necessary.
fn needs_block(block: &ast::Block, prefix: &str, context: &RewriteContext<'_>) -> bool {
    let has_attributes = block.stmts.find_non_empty().map_or(false, |first_stmt| {
        !get_attrs_from_stmt(first_stmt).is_empty()
    });

    let mut non_empty_stmts = block
        .stmts
        .iter()
        .filter(|stmt| !matches!(stmt.kind, ast::StmtKind::Empty));
    non_empty_stmts.next();

    is_unsafe_block(block)
        || non_empty_stmts.next().is_some()
        || has_attributes
        || block_contains_comment(context, block)
        || prefix.contains('\n')
}

// TODO: The expressions "veto"ed here should align with expressions that are permitted on multiple
// lines in rewrite_closure_expr#allow_multi_line. Consider refactoring to avoid this disparity.
fn veto_block(e: &ast::Expr) -> bool {
    matches!(
        e.kind,
        ast::ExprKind::Call(..)
            | ast::ExprKind::Binary(..)
            | ast::ExprKind::Cast(..)
            | ast::ExprKind::Type(..)
            | ast::ExprKind::Assign(..)
            | ast::ExprKind::AssignOp(..)
            | ast::ExprKind::Field(..)
            | ast::ExprKind::Index(..)
            | ast::ExprKind::Range(..)
            | ast::ExprKind::Try(..)
    )
}

// Rewrite closure with a single expression wrapping its body with block.
// || { #[attr] foo() } -> Block { #[attr] foo() }
fn rewrite_closure_with_block(
    body: &ast::Expr,
    prefix: &str,
    context: &RewriteContext<'_>,
    shape: Shape,
) -> Option<String> {
    let left_most = left_most_sub_expr(body);
    let veto_block = veto_block(body) && !expr_requires_semi_to_be_stmt(left_most);
    if veto_block {
        return None;
    }

    let block = ast::Block {
        stmts: vec![ast::Stmt {
            id: ast::NodeId::root(),
            kind: ast::StmtKind::Expr(ptr::P(body.clone())),
            span: body.span,
        }],
        id: ast::NodeId::root(),
        rules: ast::BlockCheckMode::Default,
        span: body
            .attrs
            .first()
            .map(|attr| attr.span.to(body.span))
            .unwrap_or(body.span),
        tokens: None,
    };
    let block =
        rewrite_block_with_visitor(context, "", &block, Some(&body.attrs), None, shape, false)?;
    Some(format!("{} {}", prefix, block))
}

// Rewrite closure with a single expression without wrapping its body with block.
fn rewrite_closure_expr(
    expr: &ast::Expr,
    prefix: &str,
    context: &RewriteContext<'_>,
    shape: Shape,
) -> Option<String> {
    fn allow_multi_line(expr: &ast::Expr) -> bool {
        match expr.kind {
            ast::ExprKind::Match(..)
            | ast::ExprKind::Async(..)
            | ast::ExprKind::Block(..)
            | ast::ExprKind::TryBlock(..)
            | ast::ExprKind::Loop(..)
            | ast::ExprKind::Struct(..)
            | ast::ExprKind::Range(..) => true,

            ast::ExprKind::AddrOf(_, _, ref expr)
            | ast::ExprKind::Box(ref expr)
            | ast::ExprKind::Try(ref expr)
            | ast::ExprKind::Unary(_, ref expr)
            | ast::ExprKind::Cast(ref expr, _) => allow_multi_line(expr),

            _ => false,
        }
    }

    // When rewriting closure's body without block, we require it to fit in a single line
    // unless it is a block-like expression or we are inside macro call.
    let veto_multiline = (!allow_multi_line(expr) && !context.inside_macro())
        || context.config.force_multiline_blocks();
    expr.rewrite(context, shape)
        .and_then(|rw| {
            if veto_multiline && rw.contains('\n') {
                None
            } else {
                Some(rw)
            }
        })
        .map(|rw| format!("{} {}", prefix, rw))
}

// Rewrite closure whose body is block.
fn rewrite_closure_block(
    block: &ast::Block,
    prefix: &str,
    context: &RewriteContext<'_>,
    shape: Shape,
) -> Option<String> {
    Some(format!("{} {}", prefix, block.rewrite(context, shape)?))
}

// Return type is (prefix, extra_offset)
fn rewrite_closure_fn_decl(
    capture: ast::CaptureBy,
    asyncness: &ast::Async,
    movability: ast::Movability,
    fn_decl: &ast::FnDecl,
    body: &ast::Expr,
    span: Span,
    context: &RewriteContext<'_>,
    shape: Shape,
) -> Option<(String, usize)> {
    let is_async = if asyncness.is_async() { "async " } else { "" };
    let mover = if capture == ast::CaptureBy::Value {
        "move "
    } else {
        ""
    };
    let immovable = if movability == ast::Movability::Static {
        "static "
    } else {
        ""
    };
    // 4 = "|| {".len(), which is overconservative when the closure consists of
    // a single expression.
    let nested_shape = shape
        .shrink_left(is_async.len() + mover.len() + immovable.len())?
        .sub_width(4)?;

    let indent_style = context.config.indent_style();

    // 1 = |
    let param_offset = nested_shape.indent + 1;
    let param_shape = match indent_style {
        IndentStyle::Block => {
            Shape::indented(shape.indent.block_indent(context.config), context.config)
        }
        IndentStyle::Visual => nested_shape.offset_left(1)?.visual_indent(0),
    };
    let ret_str = fn_decl.output.rewrite(context, param_shape)?;

    let param_items = itemize_list(
        context.snippet_provider,
        fn_decl.inputs.iter(),
        "|",
        ",",
        |param| span_lo_for_param(param),
        |param| span_hi_for_param(context, param),
        |param| param.rewrite(context, param_shape),
        context.snippet_provider.span_after(span, "|"),
        body.span.lo(),
        false,
    );
    let item_vec = param_items.collect::<Vec<_>>();
    // 1 = space between parameters and return type.
    let horizontal_budget = nested_shape.width.saturating_sub(ret_str.len() + 1);
    let tactic = definitive_tactic(
        &item_vec,
        ListTactic::HorizontalVertical,
        Separator::Comma,
        horizontal_budget,
    );
    let param_shape = match tactic {
        DefinitiveListTactic::Horizontal => param_shape.sub_width(ret_str.len() + 1)?,
        _ => param_shape,
    };

    let fmt = ListFormatting::new(param_shape, context.config)
        .tactic(tactic)
        .preserve_newline(true);
    let list_str = write_list(&item_vec, &fmt)?;
    let one_line_budget = context.budget(param_shape.indent.width());
    let multi_line_params = match indent_style {
        IndentStyle::Block => list_str.contains('\n') || list_str.len() > one_line_budget,
        _ => false,
    };
    let put_params_in_block = multi_line_params && !item_vec.is_empty();
    let param_str = if put_params_in_block {
        let trailing_comma = match context.config.trailing_comma() {
            SeparatorTactic::Never => "",
            _ => ",",
        };
        format!(
            "{}{}{}{}",
            param_shape.indent.to_string_with_newline(context.config),
            &list_str,
            trailing_comma,
            shape.indent.to_string_with_newline(context.config)
        )
    } else {
        list_str
    };
    let mut prefix = format!("{}{}{}|{}|", is_async, immovable, mover, param_str);

    if !ret_str.is_empty() {
        if prefix.contains('\n') && !put_params_in_block {
            prefix.push('\n');
            prefix.push_str(&param_offset.to_string(context.config));
        } else {
            prefix.push(' ');
        }
        prefix.push_str(&ret_str);
    }
    // 1 = space between `|...|` and body.
    let extra_offset = last_line_width(&prefix) + 1;

    Some((prefix, extra_offset))
}

// Rewriting closure which is placed at the end of the function call's arg.
// Returns `None` if the reformatted closure 'looks bad'.
pub(crate) fn rewrite_last_closure(
    context: &RewriteContext<'_>,
    expr: &ast::Expr,
    shape: Shape,
) -> Option<String> {
    if let ast::ExprKind::Closure(capture, ref is_async, movability, ref fn_decl, ref body, _) =
        expr.kind
    {
        let body = match body.kind {
            ast::ExprKind::Block(ref block, _)
                if !is_unsafe_block(block)
                    && !context.inside_macro()
                    && is_simple_block(context, block, Some(&body.attrs)) =>
            {
                stmt_expr(block.stmts.find_non_empty().unwrap()).unwrap_or(body)
            }
            _ => body,
        };
        let (prefix, extra_offset) = rewrite_closure_fn_decl(
            capture, is_async, movability, fn_decl, body, expr.span, context, shape,
        )?;
        // If the closure goes multi line before its body, do not overflow the closure.
        if prefix.contains('\n') {
            return None;
        }

        let body_shape = shape.offset_left(extra_offset)?;

        // We force to use block for the body of the closure for certain kinds of expressions.
        if is_block_closure_forced(context, body, capture) {
            return rewrite_closure_with_block(body, &prefix, context, body_shape).map(
                |body_str| {
                    match fn_decl.output {
                        ast::FnRetTy::Default(..) if body_str.lines().count() <= 7 => {
                            // If the expression can fit in a single line, we need not force block
                            // closure.  However, if the closure has a return type, then we must
                            // keep the blocks.
                            match rewrite_closure_expr(body, &prefix, context, shape) {
                                Some(ref single_line_body_str)
                                    if !single_line_body_str.contains('\n') =>
                                {
                                    single_line_body_str.clone()
                                }
                                _ => body_str,
                            }
                        }
                        _ => body_str,
                    }
                },
            );
        }

        // When overflowing the closure which consists of a single control flow expression,
        // force to use block if its condition uses multi line.
        let is_multi_lined_cond = rewrite_cond(context, body, body_shape).map_or(false, |cond| {
            cond.contains('\n') || cond.len() > body_shape.width
        });
        if is_multi_lined_cond {
            return rewrite_closure_with_block(body, &prefix, context, body_shape);
        }

        // Seems fine, just format the closure in usual manner.
        return expr.rewrite(context, shape);
    }
    None
}

/// Returns `true` if the given vector of arguments has more than one `ast::ExprKind::Closure`.
pub(crate) fn args_have_many_closure(args: &[OverflowableItem<'_>]) -> bool {
    args.iter()
        .filter_map(OverflowableItem::to_expr)
        .filter(|expr| matches!(expr.kind, ast::ExprKind::Closure(..)))
        .count()
        > 1
}

fn is_block_closure_forced(
    context: &RewriteContext<'_>,
    expr: &ast::Expr,
    capture: ast::CaptureBy,
) -> bool {
    // If we are inside macro, we do not want to add or remove block from closure body.
    if context.inside_macro() {
        false
    } else if let ast::ExprKind::Match(..) = expr.kind {
        let is_move_closure_without_brace =
            capture == ast::CaptureBy::Value && !context.snippet(expr.span).trim().starts_with('{');

        is_block_closure_forced_inner(expr) || is_move_closure_without_brace
    } else {
        is_block_closure_forced_inner(expr)
    }
}

fn is_block_closure_forced_inner(expr: &ast::Expr) -> bool {
    match expr.kind {
        ast::ExprKind::If(..)
        | ast::ExprKind::While(..)
        | ast::ExprKind::ForLoop(..)
        | ast::ExprKind::Loop(..) => true,
        ast::ExprKind::AddrOf(_, _, ref expr)
        | ast::ExprKind::Box(ref expr)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Unary(_, ref expr)
        | ast::ExprKind::Cast(ref expr, _) => is_block_closure_forced_inner(expr),
        _ => false,
    }
}

/// Does this expression require a semicolon to be treated
/// as a statement? The negation of this: 'can this expression
/// be used as a statement without a semicolon' -- is used
/// as an early-bail-out in the parser so that, for instance,
///     if true {...} else {...}
///      |x| 5
/// isn't parsed as (if true {...} else {...} | x) | 5
// From https://github.com/rust-lang/rust/blob/master/src/libsyntax/parse/classify.rs.
fn expr_requires_semi_to_be_stmt(e: &ast::Expr) -> bool {
    !matches!(
        e.kind,
        ast::ExprKind::If(..)
            | ast::ExprKind::Match(..)
            | ast::ExprKind::Block(..)
            | ast::ExprKind::While(..)
            | ast::ExprKind::Loop(..)
            | ast::ExprKind::ForLoop(..)
            | ast::ExprKind::TryBlock(..)
    )
}
