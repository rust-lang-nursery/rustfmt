//! Reorder items.
//!
//! `mod`, `extern crate` and `use` declarations are reordered in alphabetical
//! order. Trait items are reordered in pre-determined order (associated types
//! and constants comes before methods).

// FIXME(#2455): Reorder trait items.

use std::cmp::{Ord, Ordering};

use rustc_ast::ast;
use rustc_span::{symbol::sym, BytePos, Pos, Span};

use crate::config::{Config, GroupImportsTactic, ImportGranularity};
use crate::formatting::imports::{flatten_use_trees, UseSegment};
use crate::formatting::modules::{get_mod_inner_attrs, FileModMap};
use crate::formatting::{
    comment::{comment_style, contains_comment, is_first_comment_block, is_last_comment_block},
    imports::{merge_use_trees, UseTree},
    items::{is_mod_decl, rewrite_extern_crate, rewrite_mod},
    lists::{itemize_list, write_list, ListFormatting, ListItem},
    rewrite::RewriteContext,
    shape::Shape,
    source_map::LineRangeUtils,
    spanned::Spanned,
    utils::{contains_skip, mk_sp},
    visitor::FmtVisitor,
};

use super::imports::SharedPrefix;

/// Compare strings according to version sort (roughly equivalent to `strverscmp`)
pub(crate) fn compare_as_versions(left: &str, right: &str) -> Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();

    loop {
        // The strings are equal so far and not inside a number in both sides
        let (l, r) = match (left.next(), right.next()) {
            // Is this the end of both strings?
            (None, None) => return Ordering::Equal,
            // If for one, the shorter one is considered smaller
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) => (l, r),
        };
        let next_ordering = match (l.to_digit(10), r.to_digit(10)) {
            // If neither is a digit, just compare them
            (None, None) => Ord::cmp(&l, &r),
            // The one with shorter non-digit run is smaller
            // For `strverscmp` it's smaller iff next char in longer is greater than digits
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            // If both start numbers, we have to compare the numbers
            (Some(l), Some(r)) => {
                if l == 0 || r == 0 {
                    // Fraction mode: compare as if there was leading `0.`
                    let ordering = Ord::cmp(&l, &r);
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                    loop {
                        // Get next pair
                        let (l, r) = match (left.peek(), right.peek()) {
                            // Is this the end of both strings?
                            (None, None) => return Ordering::Equal,
                            // If for one, the shorter one is considered smaller
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            (Some(l), Some(r)) => (l, r),
                        };
                        // Are they digits?
                        match (l.to_digit(10), r.to_digit(10)) {
                            // If out of digits, use the stored ordering due to equal length
                            (None, None) => break Ordering::Equal,
                            // If one is shorter, it's smaller
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            // If both are digits, consume them and take into account
                            (Some(l), Some(r)) => {
                                left.next();
                                right.next();
                                let ordering = Ord::cmp(&l, &r);
                                if ordering != Ordering::Equal {
                                    return ordering;
                                }
                            }
                        }
                    }
                } else {
                    // Integer mode
                    let mut same_length_ordering = Ord::cmp(&l, &r);
                    loop {
                        // Get next pair
                        let (l, r) = match (left.peek(), right.peek()) {
                            // Is this the end of both strings?
                            (None, None) => return same_length_ordering,
                            // If for one, the shorter one is considered smaller
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            (Some(l), Some(r)) => (l, r),
                        };
                        // Are they digits?
                        match (l.to_digit(10), r.to_digit(10)) {
                            // If out of digits, use the stored ordering due to equal length
                            (None, None) => break same_length_ordering,
                            // If one is shorter, it's smaller
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            // If both are digits, consume them and take into account
                            (Some(l), Some(r)) => {
                                left.next();
                                right.next();
                                same_length_ordering = same_length_ordering.then(Ord::cmp(&l, &r));
                            }
                        }
                    }
                }
            }
        };
        if next_ordering != Ordering::Equal {
            return next_ordering;
        }
    }
}

/// Compare identifiers, trimming `r#` if present, according to version sort
pub(crate) fn compare_ident_as_versions(left: &str, right: &str) -> Ordering {
    compare_as_versions(
        left.trim_start_matches("r#"),
        right.trim_start_matches("r#"),
    )
}

pub(crate) fn compare_opt_ident_as_versions<S>(left: &Option<S>, right: &Option<S>) -> Ordering
where
    S: AsRef<str>,
{
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => compare_ident_as_versions(left.as_ref(), right.as_ref()),
    }
}

/// Choose the ordering between the given two items.
fn compare_items(a: &ast::Item, b: &ast::Item) -> Ordering {
    match (&a.kind, &b.kind) {
        (&ast::ItemKind::Mod(..), &ast::ItemKind::Mod(..)) => {
            compare_as_versions(&a.ident.as_str(), &b.ident.as_str())
        }
        (&ast::ItemKind::ExternCrate(ref a_name), &ast::ItemKind::ExternCrate(ref b_name)) => {
            // `extern crate foo as bar;`
            //               ^^^ Comparing this.
            let a_orig_name = a_name.map_or_else(|| a.ident.as_str(), rustc_span::Symbol::as_str);
            let b_orig_name = b_name.map_or_else(|| b.ident.as_str(), rustc_span::Symbol::as_str);
            let result = compare_as_versions(&a_orig_name, &b_orig_name);
            if result != Ordering::Equal {
                return result;
            }

            // `extern crate foo as bar;`
            //                      ^^^ Comparing this.
            match (a_name, b_name) {
                (Some(..), None) => Ordering::Greater,
                (None, Some(..)) => Ordering::Less,
                (None, None) => Ordering::Equal,
                (Some(..), Some(..)) => compare_as_versions(&a.ident.as_str(), &b.ident.as_str()),
            }
        }
        _ => unreachable!(),
    }
}

fn wrap_reorderable_items(
    context: &RewriteContext<'_>,
    list_items: &[ListItem],
    shape: Shape,
) -> Option<String> {
    let fmt = ListFormatting::new(shape, context.config)
        .separator("")
        .align_comments(false);
    write_list(list_items, &fmt)
}

fn rewrite_reorderable_item(
    context: &RewriteContext<'_>,
    item: &ast::Item,
    shape: Shape,
) -> Option<String> {
    match item.kind {
        ast::ItemKind::ExternCrate(..) => rewrite_extern_crate(context, item, shape),
        ast::ItemKind::Mod(..) => rewrite_mod(context, item, shape),
        _ => None,
    }
}

/// Rewrite a list of items with reordering and/or regrouping. Every item
/// in `items` must have the same `ast::ItemKind`. Whether reordering, regrouping,
/// or both are done is determined from the `context`.
fn rewrite_reorderable_or_regroupable_items(
    context: &RewriteContext<'_>,
    reorderable_items: &[&ast::Item],
    shape: Shape,
    span: Span,
) -> Option<String> {
    match reorderable_items[0].kind {
        // FIXME: Remove duplicated code.
        ast::ItemKind::Use(..) => {
            let mut normalized_items: Vec<_> = reorderable_items
                .iter()
                .filter_map(|item| UseTree::from_ast_with_normalization(context, item))
                .collect();
            let cloned = normalized_items.clone();
            // Add comments before merging.
            let list_items = itemize_list(
                context.snippet_provider,
                cloned.iter(),
                "",
                ";",
                |item| item.span().lo(),
                |item| item.span().hi(),
                |_item| Some("".to_owned()),
                span.lo(),
                span.hi(),
                false,
            );
            for (item, list_item) in normalized_items.iter_mut().zip(list_items) {
                item.list_item = Some(list_item.clone());
            }
            normalized_items = match context.config.imports_granularity() {
                ImportGranularity::Crate => merge_use_trees(normalized_items, SharedPrefix::Crate),
                ImportGranularity::Module => {
                    merge_use_trees(normalized_items, SharedPrefix::Module)
                }
                ImportGranularity::Item => flatten_use_trees(normalized_items),
                ImportGranularity::One => merge_use_trees(normalized_items, SharedPrefix::One),
                ImportGranularity::Preserve => normalized_items,
            };

            let mut regrouped_items = match context.config.group_imports() {
                GroupImportsTactic::Preserve => vec![normalized_items],
                GroupImportsTactic::StdExternalCrate => group_imports(normalized_items),
            };

            if context.config.reorder_imports() {
                regrouped_items.iter_mut().for_each(|items| items.sort())
            }

            // 4 = "use ", 1 = ";"
            let nested_shape = shape.offset_left(4)?.sub_width(1)?;
            let item_vec: Vec<_> = regrouped_items
                .into_iter()
                .filter(|use_group| !use_group.is_empty())
                .map(|use_group| {
                    let item_vec: Vec<_> = use_group
                        .into_iter()
                        .map(|use_tree| ListItem {
                            item: use_tree.rewrite_top_level(context, nested_shape),
                            ..use_tree.list_item.unwrap_or_else(ListItem::empty)
                        })
                        .collect();
                    wrap_reorderable_items(context, &item_vec, nested_shape)
                })
                .collect::<Option<Vec<_>>>()?;

            let join_string = format!("\n\n{}", shape.indent.to_string(context.config));
            Some(item_vec.join(&join_string))
        }
        _ => {
            let list_items = itemize_list(
                context.snippet_provider,
                reorderable_items.iter(),
                "",
                ";",
                |item| item.span().lo(),
                |item| item.span().hi(),
                |item| rewrite_reorderable_item(context, item, shape),
                span.lo(),
                span.hi(),
                false,
            );

            let mut item_pair_vec: Vec<_> = list_items.zip(reorderable_items.iter()).collect();
            item_pair_vec.sort_by(|a, b| compare_items(a.1, b.1));
            let item_vec: Vec<_> = item_pair_vec.into_iter().map(|pair| pair.0).collect();

            wrap_reorderable_items(context, &item_vec, shape)
        }
    }
}

fn contains_macro_use_attr(attrs: &[ast::Attribute]) -> bool {
    crate::formatting::attr::contains_name(attrs, sym::macro_use)
}

/// Divides imports into three groups, corresponding to standard, external
/// and local imports. Sorts each subgroup.
fn group_imports(uts: Vec<UseTree>) -> Vec<Vec<UseTree>> {
    let mut std_imports = Vec::new();
    let mut external_imports = Vec::new();
    let mut local_imports = Vec::new();

    for ut in uts.into_iter() {
        if ut.path.is_empty() {
            external_imports.push(ut);
            continue;
        }
        match &ut.path[0] {
            UseSegment::Ident(id, _) => match id.as_ref() {
                "std" | "alloc" | "core" => std_imports.push(ut),
                _ => external_imports.push(ut),
            },
            UseSegment::Slf(_) | UseSegment::Super(_) | UseSegment::Crate(_) => {
                local_imports.push(ut)
            }
            // These are probably illegal here
            UseSegment::Glob | UseSegment::List(_) => external_imports.push(ut),
        }
    }

    vec![std_imports, external_imports, local_imports]
}

/// A simplified version of `ast::ItemKind`.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum ReorderableItemKind {
    ExternCrate,
    Mod,
    Use,
    /// An item that cannot be reordered. Either has an unreorderable item kind
    /// or an `macro_use` attribute.
    Other,
}

impl ReorderableItemKind {
    fn from(item: &ast::Item, file_mod_map: &FileModMap<'_>) -> Self {
        match item.kind {
            _ if contains_macro_use_attr(&item.attrs) | contains_skip(&item.attrs) => {
                ReorderableItemKind::Other
            }
            ast::ItemKind::ExternCrate(..) => ReorderableItemKind::ExternCrate,
            ast::ItemKind::Mod(..)
                if is_mod_decl(item)
                    && !get_mod_inner_attrs(item, file_mod_map)
                        .map_or(false, contains_macro_use_attr) =>
            {
                ReorderableItemKind::Mod
            }
            ast::ItemKind::Use(..) => ReorderableItemKind::Use,
            _ => ReorderableItemKind::Other,
        }
    }

    fn is_same_item_kind(self, item: &ast::Item, file_mod_map: &FileModMap<'_>) -> bool {
        ReorderableItemKind::from(item, file_mod_map) == self
    }

    fn is_reorderable(self, config: &Config) -> bool {
        match self {
            ReorderableItemKind::ExternCrate => config.reorder_imports(),
            ReorderableItemKind::Mod => config.reorder_modules(),
            ReorderableItemKind::Use => config.reorder_imports(),
            ReorderableItemKind::Other => false,
        }
    }

    fn is_regroupable(self, config: &Config) -> bool {
        match self {
            ReorderableItemKind::ExternCrate
            | ReorderableItemKind::Mod
            | ReorderableItemKind::Other => false,
            ReorderableItemKind::Use => config.group_imports() != GroupImportsTactic::Preserve,
        }
    }

    fn in_group(self, config: &Config) -> bool {
        match self {
            ReorderableItemKind::ExternCrate | ReorderableItemKind::Mod => true,
            ReorderableItemKind::Use => config.group_imports() == GroupImportsTactic::Preserve,
            ReorderableItemKind::Other => false,
        }
    }
}

impl<'b, 'a: 'b> FmtVisitor<'a> {
    /// Format items with the same item kind and reorder them, regroup them, or
    /// both. If `in_group` is `true`, then the items separated by an empty line
    /// will not be reordered together.
    fn walk_reorderable_or_regroupable_items(
        &mut self,
        items: &[&ast::Item],
        item_kind: ReorderableItemKind,
        in_group: bool,
    ) -> usize {
        let mut last = self.parse_sess.lookup_line_range(items[0].span());
        let item_length = items
            .iter()
            .take_while(|ppi| {
                item_kind.is_same_item_kind(&***ppi, self.file_mod_map)
                    && (!in_group || {
                        let current = self.parse_sess.lookup_line_range(ppi.span());
                        let in_same_group = current.lo < last.hi + 2;
                        last = current;
                        in_same_group
                    })
            })
            .count();
        let items = &items[..item_length];

        let at_least_one_in_file_lines = items
            .iter()
            .any(|item| !out_of_file_lines_range!(self, item.span));

        if at_least_one_in_file_lines && !items.is_empty() {
            self.normalize_vertical_spaces = true;
            let context = self.get_context();

            let first_lo = items.first().unwrap().span().lo();
            let line_lo = self.parse_sess.line_bounds(first_lo).unwrap().start;
            let leading_snip = context.snippet(mk_sp(line_lo, first_lo));
            let lo = if contains_comment(leading_snip) {
                let comment_started = if is_last_comment_block(leading_snip) {
                    is_first_comment_block(leading_snip)
                } else {
                    true
                };
                if comment_started { line_lo } else { first_lo }
            } else {
                first_lo
            };

            let last_hi = items.last().unwrap().span().hi();
            let line_hi = self.parse_sess.line_bounds(last_hi).unwrap().end;
            let trailing_snip = context.snippet(mk_sp(last_hi, line_hi));
            let hi = if contains_comment(trailing_snip) {
                let comment_ended = if is_first_comment_block(trailing_snip) {
                    is_last_comment_block(trailing_snip.trim())
                } else {
                    true
                };
                if comment_ended {
                    // 1 = '\n'
                    line_hi - BytePos::from_usize(1)
                } else {
                    last_hi
                }
            } else {
                last_hi
            };

            let span = mk_sp(lo, hi);
            let rw = rewrite_reorderable_or_regroupable_items(
                &self.get_context(),
                items,
                self.shape(),
                span,
            );
            self.push_rewrite(span, rw);
        } else {
            for item in items {
                self.push_rewrite(item.span, None);
            }
        }

        item_length
    }

    /// Visits and format the given items. Items are reordered If they are
    /// consecutive and reorderable.
    pub(crate) fn visit_items_with_reordering(&mut self, mut items: &[&ast::Item]) {
        while !items.is_empty() {
            // If the next item is a `use`, `extern crate` or `mod`, then extract it and any
            // subsequent items that have the same item kind to be reordered within
            // `walk_reorderable_items`. Otherwise, just format the next item for output.
            let item_kind = ReorderableItemKind::from(items[0], self.file_mod_map);
            if item_kind.is_reorderable(self.config) || item_kind.is_regroupable(self.config) {
                let visited_items_num = self.walk_reorderable_or_regroupable_items(
                    items,
                    item_kind,
                    item_kind.in_group(self.config),
                );
                let (_, rest) = items.split_at(visited_items_num);
                items = rest;
            } else {
                // Reaching here means items were not reordered. There must be at least
                // one item left in `items`, so calling `unwrap()` here is safe.
                let (item, rest) = items.split_first().unwrap();
                self.visit_item(item, true);
                items = rest;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compare_as_versions() {
        use super::compare_as_versions;
        use std::cmp::Ordering;
        let mut strings: &[&'static str] = &[
            "9", "i8", "ia32", "u009", "u08", "u08", "u080", "u8", "u8", "u16", "u32", "u128",
        ];
        while !strings.is_empty() {
            let (first, tail) = strings.split_first().unwrap();
            for second in tail {
                if first == second {
                    assert_eq!(compare_as_versions(first, second), Ordering::Equal);
                    assert_eq!(compare_as_versions(second, first), Ordering::Equal);
                } else {
                    assert_eq!(compare_as_versions(first, second), Ordering::Less);
                    assert_eq!(compare_as_versions(second, first), Ordering::Greater);
                }
            }
            strings = tail;
        }
    }
    #[test]
    fn test_compare_opt_ident_as_versions() {
        use super::compare_opt_ident_as_versions;
        use std::cmp::Ordering;
        let items: &[Option<&'static str>] = &[None, Some("a"), Some("r#a"), Some("a")];
        for (p, n) in items[..items.len() - 1].iter().zip(items[1..].iter()) {
            assert!(compare_opt_ident_as_versions(p, n) != Ordering::Greater);
        }
    }
}
