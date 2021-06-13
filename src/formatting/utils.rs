use std::borrow::Cow;

use rustc_ast::ast::{
    self, Attribute, CrateSugar, MetaItem, MetaItemKind, NestedMetaItem, NodeId, Path, Visibility,
    VisibilityKind,
};
use rustc_ast::ptr;
use rustc_ast_pretty::pprust;
use rustc_span::{sym, symbol, BytePos, ExpnId, Span, Symbol, SyntaxContext};
use unicode_width::UnicodeWidthStr;

use crate::config::Config;
use crate::emitter::Verbosity;
use crate::formatting::{
    comment::{filter_normal_code, CharClasses, FullCodeCharKind, LineClasses},
    rewrite::RewriteContext,
    shape::{Indent, Shape},
    FormattedSnippet,
};
use crate::{Input, NewlineStyle, OperationSetting};

#[inline]
pub(crate) fn depr_skip_annotation() -> Symbol {
    Symbol::intern("rustfmt_skip")
}

#[inline]
pub(crate) fn skip_annotation() -> Symbol {
    Symbol::intern("rustfmt::skip")
}

pub(crate) fn rewrite_ident<'a>(context: &'a RewriteContext<'_>, ident: symbol::Ident) -> &'a str {
    context.snippet(ident.span)
}

// Computes the length of a string's last line, minus offset.
pub(crate) fn extra_offset(text: &str, shape: Shape) -> usize {
    match text.rfind('\n') {
        // 1 for newline character
        Some(idx) => text.len().saturating_sub(idx + 1 + shape.used_width()),
        None => text.len(),
    }
}

pub(crate) fn is_same_visibility(a: &Visibility, b: &Visibility) -> bool {
    match (&a.kind, &b.kind) {
        (
            VisibilityKind::Restricted { path: p, .. },
            VisibilityKind::Restricted { path: q, .. },
        ) => pprust::path_to_string(&p) == pprust::path_to_string(&q),
        (VisibilityKind::Public, VisibilityKind::Public)
        | (VisibilityKind::Inherited, VisibilityKind::Inherited)
        | (
            VisibilityKind::Crate(CrateSugar::PubCrate),
            VisibilityKind::Crate(CrateSugar::PubCrate),
        )
        | (
            VisibilityKind::Crate(CrateSugar::JustCrate),
            VisibilityKind::Crate(CrateSugar::JustCrate),
        ) => true,
        _ => false,
    }
}

// Uses Cow to avoid allocating in the common cases.
pub(crate) fn format_visibility(
    context: &RewriteContext<'_>,
    vis: &Visibility,
) -> Cow<'static, str> {
    match vis.kind {
        VisibilityKind::Public => Cow::from("pub "),
        VisibilityKind::Inherited => Cow::from(""),
        VisibilityKind::Crate(CrateSugar::PubCrate) => Cow::from("pub(crate) "),
        VisibilityKind::Crate(CrateSugar::JustCrate) => Cow::from("crate "),
        VisibilityKind::Restricted { ref path, .. } => {
            let Path { ref segments, .. } = **path;
            let mut segments_iter = segments.iter().map(|seg| rewrite_ident(context, seg.ident));
            if path.is_global() {
                segments_iter
                    .next()
                    .expect("Non-global path in pub(restricted)?");
            }
            let is_keyword = |s: &str| s == "self" || s == "super";
            let path = segments_iter.collect::<Vec<_>>().join("::");
            let in_str = if is_keyword(&path) { "" } else { "in " };

            Cow::from(format!("pub({}{}) ", in_str, path))
        }
    }
}

#[inline]
pub(crate) fn format_async(is_async: ast::Async) -> &'static str {
    match is_async {
        ast::Async::Yes { .. } => "async ",
        ast::Async::No => "",
    }
}

#[inline]
pub(crate) fn format_constness(constness: ast::Const) -> &'static str {
    match constness {
        ast::Const::Yes(..) => "const ",
        ast::Const::No => "",
    }
}

#[inline]
pub(crate) fn format_constness_right(constness: ast::Const) -> &'static str {
    match constness {
        ast::Const::Yes(..) => " const",
        ast::Const::No => "",
    }
}

#[inline]
pub(crate) fn format_defaultness(defaultness: ast::Defaultness) -> &'static str {
    match defaultness {
        ast::Defaultness::Default(..) => "default ",
        ast::Defaultness::Final => "",
    }
}

#[inline]
pub(crate) fn format_unsafety(unsafety: ast::Unsafe) -> &'static str {
    match unsafety {
        ast::Unsafe::Yes(..) => "unsafe ",
        ast::Unsafe::No => "",
    }
}

#[inline]
pub(crate) fn format_auto(is_auto: ast::IsAuto) -> &'static str {
    match is_auto {
        ast::IsAuto::Yes => "auto ",
        ast::IsAuto::No => "",
    }
}

#[inline]
pub(crate) fn format_mutability(mutability: ast::Mutability) -> &'static str {
    match mutability {
        ast::Mutability::Mut => "mut ",
        ast::Mutability::Not => "",
    }
}

#[inline]
pub(crate) fn format_extern(
    ext: ast::Extern,
    explicit_abi: bool,
    is_mod: bool,
    attrs: Option<&[ast::Attribute]>,
) -> Cow<'static, str> {
    let format_explicit_abi = |abi: &str| Cow::from(format!(r#"extern "{}" "#, abi));
    let explicit_conversion_preserves_semantics =
        || !is_mod || attrs.map_or(true, |a| a.is_empty());

    match ext {
        ast::Extern::None if !is_mod => Cow::from(""),
        ast::Extern::Explicit(ast::StrLit {
            symbol_unescaped, ..
        }) if !is_mod && symbol_unescaped == rustc_span::sym::rust => Cow::from(""),
        ast::Extern::Implicit if !explicit_abi || !explicit_conversion_preserves_semantics() => {
            Cow::from("extern ")
        }
        ast::Extern::Explicit(ast::StrLit {
            symbol_unescaped, ..
        }) if !explicit_abi && symbol_unescaped == rustc_span::sym::C => Cow::from("extern "),
        ast::Extern::None => format_explicit_abi("Rust"),
        ast::Extern::Implicit => format_explicit_abi("C"),
        ast::Extern::Explicit(ast::StrLit {
            symbol_unescaped, ..
        }) => format_explicit_abi(&symbol_unescaped.to_string()),
    }
}

#[inline]
// Transform `Vec<rustc_ast::ptr::P<T>>` into `Vec<&T>`
pub(crate) fn ptr_vec_to_ref_vec<T>(vec: &[ptr::P<T>]) -> Vec<&T> {
    vec.iter().map(|x| &**x).collect::<Vec<_>>()
}

#[inline]
pub(crate) fn filter_attributes(
    attrs: &[ast::Attribute],
    style: ast::AttrStyle,
) -> Vec<ast::Attribute> {
    attrs
        .iter()
        .filter(|a| a.style == style)
        .cloned()
        .collect::<Vec<_>>()
}

#[inline]
pub(crate) fn inner_attributes(attrs: &[ast::Attribute]) -> Vec<ast::Attribute> {
    filter_attributes(attrs, ast::AttrStyle::Inner)
}

#[inline]
pub(crate) fn outer_attributes(attrs: &[ast::Attribute]) -> Vec<ast::Attribute> {
    filter_attributes(attrs, ast::AttrStyle::Outer)
}

#[inline]
pub(crate) fn is_single_line(s: &str) -> bool {
    s.chars().find(|&c| c == '\n').is_none()
}

#[inline]
pub(crate) fn first_line_contains_single_line_comment(s: &str) -> bool {
    s.lines().next().map_or(false, |l| l.contains("//"))
}

#[inline]
pub(crate) fn last_line_contains_single_line_comment(s: &str) -> bool {
    s.lines().last().map_or(false, |l| l.contains("//"))
}

#[inline]
pub(crate) fn is_attributes_extendable(attrs_str: &str) -> bool {
    !attrs_str.contains('\n') && !last_line_contains_single_line_comment(attrs_str)
}

/// The width of the first line in s.
#[inline]
pub(crate) fn first_line_width(s: &str) -> usize {
    unicode_str_width(s.splitn(2, '\n').next().unwrap_or(""))
}

/// The width of the last line in s.
#[inline]
pub(crate) fn last_line_width(s: &str) -> usize {
    unicode_str_width(s.rsplitn(2, '\n').next().unwrap_or(""))
}

/// The total used width of the last line.
#[inline]
pub(crate) fn last_line_used_width(s: &str, offset: usize) -> usize {
    if s.contains('\n') {
        last_line_width(s)
    } else {
        offset + unicode_str_width(s)
    }
}

#[inline]
pub(crate) fn trimmed_last_line_width(s: &str) -> usize {
    unicode_str_width(match s.rfind('\n') {
        Some(n) => s[(n + 1)..].trim(),
        None => s.trim(),
    })
}

#[inline]
pub(crate) fn last_line_extendable(s: &str) -> bool {
    if s.ends_with("\"#") {
        return true;
    }
    for c in s.chars().rev() {
        match c {
            '(' | ')' | ']' | '}' | '?' | '>' => continue,
            '\n' => break,
            _ if c.is_whitespace() => continue,
            _ => return false,
        }
    }
    true
}

#[inline]
fn is_skip(meta_item: &MetaItem) -> bool {
    match meta_item.kind {
        MetaItemKind::Word => {
            let path_str = pprust::path_to_string(&meta_item.path);
            path_str == *skip_annotation().as_str() || path_str == *depr_skip_annotation().as_str()
        }
        MetaItemKind::List(ref l) => {
            meta_item.has_name(sym::cfg_attr) && l.len() == 2 && is_skip_nested(&l[1])
        }
        _ => false,
    }
}

#[inline]
fn is_skip_nested(meta_item: &NestedMetaItem) -> bool {
    match meta_item {
        NestedMetaItem::MetaItem(ref mi) => is_skip(mi),
        NestedMetaItem::Literal(_) => false,
    }
}

#[inline]
pub(crate) fn contains_skip(attrs: &[Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.meta().map_or(false, |a| is_skip(&a)))
}

#[inline]
pub(crate) fn semicolon_for_expr(context: &RewriteContext<'_>, expr: &ast::Expr) -> bool {
    // Never try to insert semicolons on expressions when we're inside
    // a macro definition - this can prevent the macro from compiling
    // when used in expression position
    if context.is_macro_def {
        return false;
    }
    match expr.kind {
        ast::ExprKind::Ret(..) | ast::ExprKind::Continue(..) | ast::ExprKind::Break(..) => {
            context.config.trailing_semicolon()
        }
        _ => false,
    }
}

#[inline]
pub(crate) fn semicolon_for_stmt(context: &RewriteContext<'_>, stmt: &ast::Stmt) -> bool {
    match stmt.kind {
        ast::StmtKind::Semi(ref expr) => match expr.kind {
            ast::ExprKind::While(..) | ast::ExprKind::Loop(..) | ast::ExprKind::ForLoop(..) => {
                false
            }
            ast::ExprKind::Break(..) | ast::ExprKind::Continue(..) | ast::ExprKind::Ret(..) => {
                context.config.trailing_semicolon()
            }
            _ => true,
        },
        ast::StmtKind::Expr(..) => false,
        _ => true,
    }
}

#[inline]
pub(crate) fn stmt_expr(stmt: &ast::Stmt) -> Option<&ast::Expr> {
    match stmt.kind {
        ast::StmtKind::Expr(ref expr) => Some(expr),
        _ => None,
    }
}

/// Returns the number of LF and CRLF respectively.
pub(crate) fn count_lf_crlf(input: &str) -> (usize, usize) {
    let mut lf = 0;
    let mut crlf = 0;
    let mut is_crlf = false;
    for c in input.as_bytes() {
        match c {
            b'\r' => is_crlf = true,
            b'\n' if is_crlf => crlf += 1,
            b'\n' => lf += 1,
            _ => is_crlf = false,
        }
    }
    (lf, crlf)
}

pub(crate) fn count_newlines(input: &str) -> usize {
    // Using bytes to omit UTF-8 decoding
    bytecount::count(input.as_bytes(), b'\n')
}

// For format_missing and last_pos, need to use the source callsite (if applicable).
// Required as generated code spans aren't guaranteed to follow on from the last span.
macro_rules! source {
    ($this:ident, $sp:expr) => {
        $sp.source_callsite()
    };
}

pub(crate) fn mk_sp(lo: BytePos, hi: BytePos) -> Span {
    Span::new(lo, hi, SyntaxContext::root())
}

pub(crate) fn mk_sp_lo_plus_one(lo: BytePos) -> Span {
    Span::new(lo, lo + BytePos(1), SyntaxContext::root())
}

// Returns `true` if the given span does not intersect with file lines.
macro_rules! out_of_file_lines_range {
    ($self:ident, $span:expr) => {
        !$self.config.file_lines().is_all()
            && !$self
                .config
                .file_lines()
                .intersects(&$self.parse_sess.lookup_line_range($span))
    };
}

macro_rules! skip_out_of_file_lines_range {
    ($self:ident, $span:expr) => {
        if out_of_file_lines_range!($self, $span) {
            return None;
        }
    };
}

macro_rules! skip_out_of_file_lines_range_visitor {
    ($self:ident, $span:expr) => {
        if out_of_file_lines_range!($self, $span) {
            $self.push_rewrite($span, None);
            return;
        }
    };
}

// Wraps String in an Option. Returns Some when the string adheres to the
// Rewrite constraints defined for the Rewrite trait and None otherwise.
pub(crate) fn wrap_str(s: String, max_width: usize, shape: Shape) -> Option<String> {
    if is_valid_str(&filter_normal_code(&s), max_width, shape) {
        Some(s)
    } else {
        None
    }
}

fn is_valid_str(snippet: &str, max_width: usize, shape: Shape) -> bool {
    if !snippet.is_empty() {
        // First line must fits with `shape.width`.
        if first_line_width(snippet) > shape.width {
            return false;
        }
        // If the snippet does not include newline, we are done.
        if is_single_line(snippet) {
            return true;
        }
        // The other lines must fit within the maximum width.
        if snippet
            .lines()
            .skip(1)
            .any(|line| unicode_str_width(line) > max_width)
        {
            return false;
        }
        // A special check for the last line, since the caller may
        // place trailing characters on this line.
        if last_line_width(snippet) > shape.used_width() + shape.width {
            return false;
        }
    }
    true
}

#[inline]
pub(crate) fn colon_spaces(config: &Config) -> &'static str {
    let before = config.space_before_colon();
    let after = config.space_after_colon();
    match (before, after) {
        (true, true) => " : ",
        (true, false) => " :",
        (false, true) => ": ",
        (false, false) => ":",
    }
}

#[inline]
pub(crate) fn left_most_sub_expr(e: &ast::Expr) -> &ast::Expr {
    match e.kind {
        ast::ExprKind::Call(ref e, _)
        | ast::ExprKind::Binary(_, ref e, _)
        | ast::ExprKind::Cast(ref e, _)
        | ast::ExprKind::Type(ref e, _)
        | ast::ExprKind::Assign(ref e, _, _)
        | ast::ExprKind::AssignOp(_, ref e, _)
        | ast::ExprKind::Field(ref e, _)
        | ast::ExprKind::Index(ref e, _)
        | ast::ExprKind::Range(Some(ref e), _, _)
        | ast::ExprKind::Try(ref e) => left_most_sub_expr(e),
        _ => e,
    }
}

#[inline]
pub(crate) fn starts_with_newline(s: &str) -> bool {
    s.starts_with('\n') || s.starts_with("\r\n")
}

#[inline]
pub(crate) fn first_line_ends_with(s: &str, c: char) -> bool {
    s.lines().next().map_or(false, |l| l.ends_with(c))
}

// States whether an expression's last line exclusively consists of closing
// parens, braces, and brackets in its idiomatic formatting.
pub(crate) fn is_block_expr(context: &RewriteContext<'_>, expr: &ast::Expr, repr: &str) -> bool {
    match expr.kind {
        ast::ExprKind::MacCall(..)
        | ast::ExprKind::Call(..)
        | ast::ExprKind::MethodCall(..)
        | ast::ExprKind::Array(..)
        | ast::ExprKind::Struct(..)
        | ast::ExprKind::While(..)
        | ast::ExprKind::If(..)
        | ast::ExprKind::Block(..)
        | ast::ExprKind::ConstBlock(..)
        | ast::ExprKind::Async(..)
        | ast::ExprKind::Loop(..)
        | ast::ExprKind::ForLoop(..)
        | ast::ExprKind::TryBlock(..)
        | ast::ExprKind::Match(..) => repr.contains('\n'),
        ast::ExprKind::Paren(ref expr)
        | ast::ExprKind::Binary(_, _, ref expr)
        | ast::ExprKind::Index(_, ref expr)
        | ast::ExprKind::Unary(_, ref expr)
        | ast::ExprKind::Closure(_, _, _, _, ref expr, _)
        | ast::ExprKind::Try(ref expr)
        | ast::ExprKind::Yield(Some(ref expr)) => is_block_expr(context, expr, repr),
        // This can only be a string lit
        ast::ExprKind::Lit(_) => {
            repr.contains('\n') && trimmed_last_line_width(repr) <= context.config.tab_spaces()
        }
        ast::ExprKind::AddrOf(..)
        | ast::ExprKind::Assign(..)
        | ast::ExprKind::AssignOp(..)
        | ast::ExprKind::Await(..)
        | ast::ExprKind::Box(..)
        | ast::ExprKind::Break(..)
        | ast::ExprKind::Cast(..)
        | ast::ExprKind::Continue(..)
        | ast::ExprKind::Err
        | ast::ExprKind::Field(..)
        | ast::ExprKind::InlineAsm(..)
        | ast::ExprKind::LlvmInlineAsm(..)
        | ast::ExprKind::Let(..)
        | ast::ExprKind::Path(..)
        | ast::ExprKind::Range(..)
        | ast::ExprKind::Repeat(..)
        | ast::ExprKind::Ret(..)
        | ast::ExprKind::Tup(..)
        | ast::ExprKind::Type(..)
        | ast::ExprKind::Yield(None)
        | ast::ExprKind::Underscore => false,
    }
}

/// Removes trailing spaces from the specified snippet. We do not remove spaces
/// inside strings or comments.
pub(crate) fn remove_trailing_white_spaces(text: &str) -> String {
    let mut buffer = String::with_capacity(text.len());
    let mut space_buffer = String::with_capacity(128);
    for (char_kind, c) in CharClasses::new(text.chars()) {
        match c {
            '\n' => {
                if char_kind == FullCodeCharKind::InString {
                    buffer.push_str(&space_buffer);
                }
                space_buffer.clear();
                buffer.push('\n');
            }
            _ if c.is_whitespace() => {
                space_buffer.push(c);
            }
            _ => {
                if !space_buffer.is_empty() {
                    buffer.push_str(&space_buffer);
                    space_buffer.clear();
                }
                buffer.push(c);
            }
        }
    }
    buffer
}

/// Indent each line according to the specified `indent`.
/// e.g.
///
/// ```rust,compile_fail
/// foo!{
/// x,
/// y,
/// foo(
///     a,
///     b,
///     c,
/// ),
/// }
/// ```
///
/// will become
///
/// ```rust,compile_fail
/// foo!{
///     x,
///     y,
///     foo(
///         a,
///         b,
///         c,
///     ),
/// }
/// ```
pub(crate) fn trim_left_preserve_layout(
    orig: &str,
    indent: Indent,
    config: &Config,
    inside_doc_comment: bool,
) -> Option<String> {
    let mut lines = LineClasses::new(orig);
    let first_line = lines
        .next()
        .map(|(_, s)| trim_end_unless_two_whitespaces(&s, inside_doc_comment).to_owned())?;
    let mut trimmed_lines = Vec::with_capacity(16);

    let mut veto_trim = false;
    let min_prefix_space_width = lines
        .filter_map(|(kind, line)| {
            let mut trimmed = true;
            let prefix_space_width = if is_empty_line(&line) {
                None
            } else {
                Some(get_prefix_space_width(config, &line))
            };

            // just InString{Commented} in order to allow the start of a string to be indented
            let new_veto_trim_value = (kind == FullCodeCharKind::InString
                || kind == FullCodeCharKind::InStringCommented)
                && !line.ends_with('\\');
            let code_block = kind.is_code_block();
            let line = if veto_trim || new_veto_trim_value || code_block {
                veto_trim = new_veto_trim_value;
                trimmed = false;
                line
            } else {
                trim_end_unless_two_whitespaces(line.trim_start(), inside_doc_comment).to_owned()
            };
            trimmed_lines.push((trimmed, line, prefix_space_width));

            // Because there is a veto against trimming and indenting lines within a string,
            // such lines should not be taken into account when computing the minimum.
            match kind {
                FullCodeCharKind::InStringCommented | FullCodeCharKind::EndStringCommented => None,
                FullCodeCharKind::InString | FullCodeCharKind::EndString => None,
                _ => prefix_space_width,
            }
        })
        .min()?;

    Some(
        first_line
            + "\n"
            + &trimmed_lines
                .iter()
                .map(
                    |&(trimmed, ref line, prefix_space_width)| match prefix_space_width {
                        _ if !trimmed => line.to_owned(),
                        Some(original_indent_width) => {
                            let new_indent_width = indent.width()
                                + original_indent_width.saturating_sub(min_prefix_space_width);
                            let new_indent = Indent::from_width(config, new_indent_width);
                            format!("{}{}", new_indent.to_string(config), line)
                        }
                        None => String::new(),
                    },
                )
                .collect::<Vec<_>>()
                .join("\n"),
    )
}

/// Trim trailing whitespace unless it consists of two or more whitespaces and is in a doc comment.
/// This is, needed to preserve Markdown's double-space line break syntax.
pub(crate) fn trim_end_unless_two_whitespaces(s: &str, is_doc_comment: bool) -> &str {
    if is_doc_comment && s.ends_with("  ") {
        s
    } else {
        s.trim_end()
    }
}

/// Based on the given line, determine if the next line can be indented or not.
/// This allows to preserve the indentation of multi-line literals.
pub(crate) fn indent_next_line(kind: FullCodeCharKind) -> bool {
    !(kind.is_string() || kind.is_commented_string())
}

pub(crate) fn is_empty_line(s: &str) -> bool {
    s.is_empty() || s.chars().all(char::is_whitespace)
}

fn get_prefix_space_width(config: &Config, s: &str) -> usize {
    let mut width = 0;
    for c in s.chars() {
        match c {
            ' ' => width += 1,
            '\t' => width += config.tab_spaces(),
            _ => return width,
        }
    }
    width
}

pub(crate) fn tab_to_spaces(s: &str, tab_spaces: usize) -> usize {
    s.chars()
        .map(|s| if s == '\t' { tab_spaces } else { 1 })
        .sum()
}

pub(crate) trait NodeIdExt {
    fn root() -> Self;
}

impl NodeIdExt for NodeId {
    fn root() -> NodeId {
        NodeId::placeholder_from_expn_id(ExpnId::root())
    }
}

pub(crate) fn unicode_str_width(s: &str) -> usize {
    s.width()
}

/// Format the given snippet. The snippet is expected to be *complete* code.
/// When we cannot parse the given snippet, this function returns `None`.
pub(crate) fn format_snippet(
    snippet: &str,
    config: &Config,
    is_macro_def: bool,
) -> Option<FormattedSnippet> {
    let mut config = config.clone();
    std::panic::catch_unwind(move || {
        config.set().hide_parse_errors(true);

        let result = {
            let input = Input::Text(snippet.into());
            crate::format_input_inner(
                input,
                &config,
                OperationSetting {
                    verbosity: Verbosity::Quiet,
                    ..OperationSetting::default()
                },
                is_macro_def,
            )
        };
        match result {
            Ok(report) if !report.has_errors() => {
                match report.format_result_as_rc().borrow().iter().next() {
                    Some((_, format_result))
                        if format_result.all_errors().count() == 0
                            && !format_result.formatted_text().is_empty() =>
                    {
                        Some(format_result.formatted_snippet().clone())
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    })
    // Discard panics encountered while formatting the snippet
    // The ? operator is needed to remove the extra Option
    .ok()?
}

/// Format the given code block. Mainly targeted for code block in comment.
/// The code block may be incomplete (i.e., parser may be unable to parse it).
/// To avoid panic in parser, we wrap the code block with a dummy function.
/// The returned code block does **not** end with newline.
pub(crate) fn format_code_block(
    code_snippet: &str,
    config: &Config,
    is_macro_def: bool,
) -> Option<FormattedSnippet> {
    const FN_MAIN_PREFIX: &str = "fn main() {\n";

    fn enclose_in_main_block(s: &str, config: &Config) -> String {
        let indent = Indent::from_width(config, config.tab_spaces());
        let mut result = String::with_capacity(s.len() * 2);
        result.push_str(FN_MAIN_PREFIX);
        let mut need_indent = true;
        for (kind, line) in LineClasses::new(s) {
            if need_indent {
                result.push_str(&indent.to_string(config));
            }
            result.push_str(&line);
            result.push('\n');
            need_indent = indent_next_line(kind);
        }
        result.push('}');
        result
    }

    // Wrap the given code block with `fn main()` if it does not have one.
    let snippet = enclose_in_main_block(code_snippet, config);
    let mut result = String::with_capacity(snippet.len());
    let mut is_first = true;

    // While formatting the code, ignore the config's newline style setting and always use "\n"
    // instead of "\r\n" for the newline characters. This is ok because the output here is
    // not directly outputted by rustfmt command, but used by the comment formatter's input.
    // We have output-file-wide "\n" ==> "\r\n" conversion process after here if it's necessary.
    let mut config_with_unix_newline = config.clone();
    config_with_unix_newline
        .set()
        .newline_style(NewlineStyle::Unix);
    let mut formatted = format_snippet(&snippet, &config_with_unix_newline, is_macro_def)?;
    // Remove wrapping main block
    formatted.unwrap_code_block();

    // Trim "fn main() {" on the first line and "}" on the last line,
    // then unindent the whole code block.
    let block_len = formatted
        .snippet
        .rfind('}')
        .unwrap_or_else(|| formatted.snippet.len());
    let mut is_indented = true;
    let indent_str = Indent::from_width(config, config.tab_spaces()).to_string(config);
    for (kind, ref line) in LineClasses::new(&formatted.snippet[FN_MAIN_PREFIX.len()..block_len]) {
        if !is_first {
            result.push('\n');
        } else {
            is_first = false;
        }
        let trimmed_line = if !is_indented {
            line
        } else if line.len() > indent_str.len() {
            // Make sure that the line has leading whitespaces.
            if line.starts_with(indent_str.as_ref()) {
                let offset = if config.hard_tabs() {
                    1
                } else {
                    config.tab_spaces()
                };
                &line[offset..]
            } else {
                line
            }
        } else {
            line
        };
        result.push_str(trimmed_line);
        is_indented = indent_next_line(kind);
    }
    Some(FormattedSnippet {
        snippet: result,
        non_formatted_ranges: formatted.non_formatted_ranges,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_remove_trailing_white_spaces() {
        let s = "    r#\"\n        test\n    \"#";
        assert_eq!(remove_trailing_white_spaces(&s), s);
    }

    #[test]
    fn test_trim_left_preserve_layout() {
        let s = "aaa\n\tbbb\n    ccc";
        let config = Config::default();
        let indent = Indent::new(4, 0);
        assert_eq!(
            trim_left_preserve_layout(&s, indent, &config, false),
            Some("aaa\n    bbb\n    ccc".to_string())
        );
    }

    #[test]
    fn test_trim_left_preserve_layout_no_trim_end_in_doc_comment() {
        let s = "aaa    \n\tbbb    \n    ccc    ";
        let config = Config::default();
        let indent = Indent::new(4, 0);
        assert_eq!(
            trim_left_preserve_layout(&s, indent, &config, true),
            Some("aaa    \n    bbb    \n    ccc    ".to_string())
        );
    }

    #[test]
    fn test_no_panic_on_format_snippet_and_format_code_block() {
        // `format_snippet()` and `format_code_block()` should not panic
        // even when we cannot parse the given snippet.
        let snippet = "let";
        assert!(format_snippet(snippet, &Config::default(), false).is_none());
        assert!(format_code_block(snippet, &Config::default(), false).is_none());
    }

    fn test_format_inner<F>(formatter: F, input: &str, expected: &str) -> bool
    where
        F: Fn(&str, &Config, bool /* is_code_block */) -> Option<FormattedSnippet>,
    {
        let output = formatter(input, &Config::default(), false);
        output.is_some() && output.unwrap().snippet == expected
    }

    #[test]
    fn test_format_snippet() {
        let snippet = "fn main() { println!(\"hello, world\"); }";
        #[cfg(not(windows))]
        let expected = "fn main() {\n    \
                        println!(\"hello, world\");\n\
                        }\n";
        #[cfg(windows)]
        let expected = "fn main() {\r\n    \
                        println!(\"hello, world\");\r\n\
                        }\r\n";
        assert!(test_format_inner(format_snippet, snippet, expected));
    }

    #[test]
    fn test_format_code_block() {
        // simple code block
        let code_block = "let x=3;";
        let expected = "let x = 3;";
        assert!(test_format_inner(format_code_block, code_block, expected));

        // more complex code block, taken from chains.rs.
        let code_block =
"let (nested_shape, extend) = if !parent_rewrite_contains_newline && is_continuable(&parent) {
(
chain_indent(context, shape.add_offset(parent_rewrite.len())),
context.config.indent_style() == IndentStyle::Visual || is_small_parent,
)
} else if is_block_expr(context, &parent, &parent_rewrite) {
match context.config.indent_style() {
// Try to put the first child on the same line with parent's last line
IndentStyle::Block => (parent_shape.block_indent(context.config.tab_spaces()), true),
// The parent is a block, so align the rest of the chain with the closing
// brace.
IndentStyle::Visual => (parent_shape, false),
}
} else {
(
chain_indent(context, shape.add_offset(parent_rewrite.len())),
false,
)
};
";
        let expected =
"let (nested_shape, extend) = if !parent_rewrite_contains_newline && is_continuable(&parent) {
    (
        chain_indent(context, shape.add_offset(parent_rewrite.len())),
        context.config.indent_style() == IndentStyle::Visual || is_small_parent,
    )
} else if is_block_expr(context, &parent, &parent_rewrite) {
    match context.config.indent_style() {
        // Try to put the first child on the same line with parent's last line
        IndentStyle::Block => (parent_shape.block_indent(context.config.tab_spaces()), true),
        // The parent is a block, so align the rest of the chain with the closing
        // brace.
        IndentStyle::Visual => (parent_shape, false),
    }
} else {
    (
        chain_indent(context, shape.add_offset(parent_rewrite.len())),
        false,
    )
};";
        assert!(test_format_inner(format_code_block, code_block, expected));

        #[rustfmt::skip]
        let code_block = "this_line_is_100_characters_long_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx(x, y, z);";
        #[rustfmt::skip]
        let expected = "this_line_is_100_characters_long_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx(x, y, z);";
        assert!(test_format_inner(format_code_block, code_block, expected));
    }
}
