use rustc_span::{BytePos, Span};

use crate::config::{file_lines::FileLines, FileName};

use crate::formatting::{
    comment::{is_last_comment_block, rewrite_comment, CodeCharKind, CommentCodeSlices},
    shape::{Indent, Shape},
    source_map::LineRangeUtils,
    utils::{
        count_lf_crlf, count_newlines, last_line_contains_single_line_comment, last_line_width,
        mk_sp,
    },
    visitor::FmtVisitor,
};

struct SnippetStatus {
    /// An offset to the current line from the beginning of the original snippet.
    line_start: usize,
    /// A length of trailing whitespaces on the current line.
    last_wspace: Option<usize>,
    /// The current line number.
    cur_line: usize,
}

impl SnippetStatus {
    fn new(cur_line: usize) -> Self {
        SnippetStatus {
            line_start: 0,
            last_wspace: None,
            cur_line,
        }
    }
}

impl<'a> FmtVisitor<'a> {
    fn output_at_start(&self) -> bool {
        self.buffer.is_empty()
    }

    pub(crate) fn format_missing(&mut self, end: BytePos) {
        // HACK(topecongiro): we use `format_missing()` to extract a missing comment between
        // a macro (or similar) and a trailing semicolon. Here we just try to avoid calling
        // `format_missing_inner` in the common case where there is no such comment.
        // This is a hack, ideally we should fix a possible bug in `format_missing_inner`
        // or refactor `visit_mac` and `rewrite_macro`, but this should suffice to fix the
        // issue (#2727).
        let missing_snippet = self.snippet(mk_sp(self.last_pos, end));
        if missing_snippet.trim() == ";" {
            self.push_str(";");
            self.last_pos = end;
            return;
        }
        self.format_missing_inner(end, true, |this, last_snippet, _| {
            this.push_str(last_snippet)
        });
        self.normalize_vertical_spaces = false;
    }

    pub(crate) fn format_missing_with_indent(&mut self, end: BytePos) {
        let config = self.config;
        self.format_missing_inner(end, true, |this, last_snippet, snippet| {
            this.push_str(last_snippet.trim_end());
            if last_snippet == snippet && !this.output_at_start() {
                // No new lines in the snippet.
                this.push_str("\n");
            }
            let indent = this.block_indent.to_string(config);
            this.push_str(&indent);
        });
        self.normalize_vertical_spaces = false;
    }

    pub(crate) fn format_missing_no_indent(&mut self, end: BytePos) {
        self.format_missing_inner(end, true, |this, last_snippet, _| {
            this.push_str(last_snippet.trim_end());
        });
        self.normalize_vertical_spaces = false;
    }

    pub(crate) fn format_missing_before_skip(&mut self, end: BytePos) {
        self.format_missing_inner(end, false, |this, last_snippet, _| {
            this.push_str(last_snippet);
        });
        self.normalize_vertical_spaces = false;
    }

    fn format_missing_inner<F: Fn(&mut FmtVisitor<'_>, &str, &str)>(
        &mut self,
        end: BytePos,
        trim_last_line: bool, // Keep last line contents when snippet is empty
        process_last_snippet: F,
    ) {
        let start = self.last_pos;

        if start == end {
            // Do nothing if this is the beginning of the file.
            if !self.output_at_start() {
                process_last_snippet(self, "", "");
            }
            return;
        }

        assert!(
            start < end,
            "Request to format inverted span: {}",
            self.parse_sess.span_to_debug_info(mk_sp(start, end)),
        );

        self.last_pos = end;
        let span = mk_sp(start, end);
        let snippet = self.snippet(span);

        // Do nothing for spaces in the beginning of the file
        let is_start_span =
            (start == BytePos(0) && end.0 as usize == snippet.len()) || self.is_start_span(span);
        if is_start_span && snippet.trim().is_empty() {
            return;
        }

        if snippet.trim().is_empty() && !out_of_file_lines_range!(self, span) {
            // Keep vertical spaces within range.
            self.push_vertical_spaces(count_newlines(snippet));
            if trim_last_line {
                process_last_snippet(self, "", snippet);
            } else {
                let last_nl = snippet.rfind('\n');
                let first = if let Some(last) = last_nl {
                    last + 1
                } else {
                    0
                };
                process_last_snippet(self, &snippet[first..], "");
            }
        } else {
            self.write_snippet(span, &process_last_snippet);
        }
    }

    fn normalize_newline_count(&self, mut newline_count: usize) -> usize {
        let offset = self.buffer.chars().rev().take_while(|c| *c == '\n').count();
        let newline_upper_bound = self.config.blank_lines_upper_bound() + 1;
        let newline_lower_bound = self.config.blank_lines_lower_bound() + 1;

        if newline_count + offset > newline_upper_bound {
            if offset >= newline_upper_bound {
                newline_count = 0;
            } else {
                newline_count = newline_upper_bound - offset;
            }
        } else if newline_count + offset < newline_lower_bound {
            if offset >= newline_lower_bound {
                newline_count = 0;
            } else {
                newline_count = newline_lower_bound - offset;
            }
        }

        newline_count
    }

    fn push_vertical_spaces(&mut self, mut newline_count: usize) {
        if self.normalize_vertical_spaces {
            newline_count = self.normalize_newline_count(newline_count);
        } else if newline_count < 1 {
            newline_count = 1;
        }

        let blank_lines = "\n".repeat(newline_count);
        self.push_str(&blank_lines);
    }

    fn write_snippet<F>(&mut self, span: Span, process_last_snippet: F)
    where
        F: Fn(&mut FmtVisitor<'_>, &str, &str),
    {
        let snippet = self.snippet(span);

        debug!("write_snippet `{}`", snippet);

        self.write_snippet_inner(snippet, span, process_last_snippet);
    }

    fn write_snippet_inner<F>(&mut self, snippet: &str, span: Span, process_last_snippet: F)
    where
        F: Fn(&mut FmtVisitor<'_>, &str, &str),
    {
        // Trim whitespace from the right hand side of each line.
        // Annoyingly, the library functions for splitting by lines etc. are not
        // quite right, so we must do it ourselves.
        let line = self.parse_sess.line_of_byte_pos(span.lo());
        let file_name = &self.parse_sess.span_to_filename(span);
        let mut status = SnippetStatus::new(line);

        let slice_within_file_lines_range =
            |file_lines: FileLines, cur_line, s| -> (usize, usize, bool) {
                let (lf_count, crlf_count) = count_lf_crlf(s);
                let newline_count = lf_count + crlf_count;
                let within_file_lines_range = file_lines.contains_range(
                    file_name,
                    cur_line,
                    // if a newline character is at the end of the slice, then the number of
                    // newlines needs to be decreased by 1 so that the range checked against
                    // the file_lines is the visual range one would expect.
                    cur_line + newline_count - if s.ends_with('\n') { 1 } else { 0 },
                );
                (lf_count, crlf_count, within_file_lines_range)
            };
        let last_line_offset = if last_line_contains_single_line_comment(&self.buffer) {
            0
        } else {
            last_line_width(&&self.buffer)
        };
        for (kind, offset, subslice) in
            CommentCodeSlices::with_offset(snippet, last_line_offset, self.config.tab_spaces())
        {
            debug!("{:?}: {:?}", kind, subslice);

            let (lf_count, crlf_count, within_file_lines_range) =
                slice_within_file_lines_range(self.config.file_lines(), status.cur_line, subslice);
            let newline_count = lf_count + crlf_count;
            if CodeCharKind::Comment == kind && within_file_lines_range {
                // 1: comment.
                self.process_comment(&mut status, snippet, offset, subslice);
            } else if subslice.trim().is_empty() && newline_count > 0 && within_file_lines_range {
                // 2: blank lines.
                self.push_vertical_spaces(newline_count);
                status.cur_line += newline_count;
                status.line_start = offset + lf_count + crlf_count * 2;
            } else {
                // 3: code which we failed to format or which is not within file-lines range.
                self.process_missing_code(&mut status, snippet, subslice, offset, file_name);
            }
        }

        let last_snippet = &snippet[status.line_start..];
        let (_, _, within_file_lines_range) =
            slice_within_file_lines_range(self.config.file_lines(), status.cur_line, last_snippet);
        if within_file_lines_range {
            process_last_snippet(self, last_snippet, snippet);
        } else {
            // just append what's left
            self.push_str(last_snippet);
        }
    }

    fn process_comment(
        &mut self,
        status: &mut SnippetStatus,
        snippet: &str,
        offset: usize,
        subslice: &str,
    ) {
        let last_char = self
            .buffer
            .chars()
            .rev()
            .find(|rev_c| ![' ', '\t'].contains(rev_c));

        let fix_indent = last_char.map_or(true, |rev_c| ['{', '\n'].contains(&rev_c));

        let comment_indent = if fix_indent {
            if let Some('{') = last_char {
                self.push_str("\n");
            }
            let indent_str = self.block_indent.to_string(self.config);
            self.push_str(&indent_str);
            self.block_indent
        } else {
            self.push_str(" ");
            Indent::from_width(self.config, last_line_width(&self.buffer))
        };

        let comment_width = ::std::cmp::min(
            self.config.comment_width(),
            self.config.max_width() - self.block_indent.width(),
        );
        let comment_shape = Shape::legacy(comment_width, comment_indent);
        let comment_str = rewrite_comment(subslice, false, comment_shape, self.config)
            .unwrap_or_else(|| String::from(subslice));
        self.push_str(&comment_str);

        status.last_wspace = None;
        status.line_start = offset + subslice.len();

        // Add a newline:
        // - if there isn't one already
        // - otherwise, only if the last line is a line comment
        if status.line_start <= snippet.len() {
            match snippet[status.line_start..]
                .chars()
                // skip trailing whitespaces
                .find(|c| !(*c == ' ' || *c == '\t'))
            {
                Some('\n') | Some('\r') => {
                    if !is_last_comment_block(subslice) {
                        self.push_str("\n");
                    }
                }
                _ => self.push_str("\n"),
            }
        }

        status.cur_line += count_newlines(subslice);
    }

    fn process_missing_code(
        &mut self,
        status: &mut SnippetStatus,
        snippet: &str,
        subslice: &str,
        offset: usize,
        file_name: &FileName,
    ) {
        for (mut i, c) in subslice.char_indices() {
            i += offset;

            if c == '\n' {
                let skip_this_line = !self
                    .config
                    .file_lines()
                    .contains_line(file_name, status.cur_line);
                if skip_this_line {
                    status.last_wspace = None;
                }

                if let Some(lw) = status.last_wspace {
                    self.push_str(&snippet[status.line_start..lw]);
                    self.push_str("\n");
                    status.last_wspace = None;
                } else {
                    self.push_str(&snippet[status.line_start..=i]);
                }

                status.cur_line += 1;
                status.line_start = i + 1;
            } else if c.is_whitespace() && status.last_wspace.is_none() {
                status.last_wspace = Some(i);
            } else {
                status.last_wspace = None;
            }
        }

        let remaining = snippet[status.line_start..subslice.len() + offset].trim();
        if !remaining.is_empty() {
            self.push_str(&self.block_indent.to_string(self.config));
            self.push_str(remaining);
            status.line_start = subslice.len() + offset;
        }
    }
}
