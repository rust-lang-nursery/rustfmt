// High level formatting functions.

use std::time::{Duration, Instant};

use rustc_ast::{ast, AstLike};
use rustc_span::symbol;

pub(crate) use syntux::session::ParseSess;

use crate::config::{Config, FileName};
use crate::formatting::{
    comment::{CharClasses, FullCodeCharKind},
    generated::is_generated_file,
    modules::{FileModMap, Module},
    newline_style::apply_newline_style,
    report::NonFormattedRange,
    syntux::parser::{DirectoryOwnership, Parser, ParserError},
    utils::{contains_skip, count_newlines},
    visitor::FmtVisitor,
};
use crate::{
    result::{ErrorKind, FormatError, OperationError},
    FormatReport, FormatResult, Input, OperationSetting, Verbosity,
};

#[macro_use]
mod utils;

mod attr;
mod chains;
mod closures;
mod comment;
mod expr;
mod generated;
mod imports;
mod items;
mod lists;
mod macros;
mod matches;
mod missed_spans;
pub(crate) mod modules;
mod newline_style;
mod overflow;
mod pairs;
mod patterns;
mod reorder;
mod rewrite;
mod shape;
mod skip;
pub(crate) mod source_map;
mod spanned;
mod stmt;
mod string;
mod syntux;
mod types;
mod vertical;
pub(crate) mod visitor;

pub(crate) mod report;

pub(crate) fn format_input_inner(
    input: Input,
    config: &Config,
    operation_setting: OperationSetting,
    is_macro_def: bool,
) -> Result<FormatReport, OperationError> {
    if !config.version_meets_requirement() {
        return Err(OperationError::VersionMismatch);
    }

    rustc_span::with_session_globals(config.edition().into(), || {
        format_project(input, config, operation_setting, is_macro_def)
    })
}

fn format_project(
    input: Input,
    config: &Config,
    operation_setting: OperationSetting,
    is_macro_def: bool,
) -> Result<FormatReport, OperationError> {
    let mut timer = Timer::start();

    let format_report = FormatReport::new();

    let main_file = input.file_name();
    let input_is_stdin = main_file == FileName::Stdin;

    let mut parse_session = ParseSess::new(config)?;
    if !operation_setting.recursive && parse_session.ignore_file(&main_file) {
        format_report.add_ignored_file(main_file);
        return Ok(format_report);
    }

    // Parse the crate.
    let directory_ownership = input.to_directory_ownership(operation_setting.recursive);
    let original_snippet = if let Input::Text(ref str) = input {
        Some(str.to_owned())
    } else {
        None
    };

    let krate = match Parser::parse_crate(input, &parse_session) {
        Ok(krate) => krate,
        Err(e) => {
            return Err(OperationError::ParseError {
                input: main_file,
                is_panic: e == ParserError::ParsePanicError,
            });
        }
    };

    if !operation_setting.recursive {
        // Suppress error output for sub-modules if we are not in recurisve mode.
        parse_session.set_silent_emitter();
    }

    let files = modules::ModResolver::new(
        &parse_session,
        directory_ownership.unwrap_or(DirectoryOwnership::UnownedViaBlock),
        !input_is_stdin && operation_setting.recursive,
    )
    .visit_crate(&krate)?;

    timer = timer.done_parsing();

    // Suppress error output if we have to do any further parsing.
    parse_session.set_silent_emitter();

    for (path, module) in &files {
        let should_ignore = (!input_is_stdin && parse_session.ignore_file(&path))
            || (!config.format_generated_files()
                && is_generated_file(&path, original_snippet.as_ref()));

        if (!operation_setting.recursive && path != &main_file) || should_ignore {
            continue;
        }
        if contains_skip(module.attrs()) {
            continue;
        }

        should_emit_verbose(input_is_stdin, operation_setting.verbosity, || {
            println!("Formatting {}", path)
        });
        format_file(
            &parse_session,
            config,
            &krate,
            path,
            &module,
            &format_report,
            &files,
            original_snippet.clone(),
            is_macro_def,
        )?;
    }
    timer = timer.done_formatting();

    should_emit_verbose(input_is_stdin, operation_setting.verbosity, || {
        println!(
            "Spent {0:.3} secs in the parsing phase, and {1:.3} secs in the formatting phase",
            timer.get_parse_time(),
            timer.get_format_time(),
        )
    });

    // Debug messages with skipped ranges for all input files
    for (filename, format_result) in format_report.format_result_as_rc().borrow().iter() {
        let skipped_ranges = &format_result.formatted_snippet().non_formatted_ranges;
        debug!(
            "format_project: filename \"{:?}\" skipped_ranges \"{:?}\"",
            filename, skipped_ranges
        );
    }

    Ok(format_report)
}

fn format_file(
    parse_session: &ParseSess,
    config: &Config,
    krate: &ast::Crate,
    path: &FileName,
    module: &Module<'_>,
    report: &FormatReport,
    file_mod_map: &FileModMap<'_>,
    original_snippet: Option<String>,
    is_macro_def: bool,
) -> Result<(), OperationError> {
    let snippet_provider = parse_session.snippet_provider(module.span);
    let mut visitor = FmtVisitor::from_parse_sess(
        &parse_session,
        config,
        &snippet_provider,
        file_mod_map,
        report.clone(),
    );
    visitor.is_macro_def = is_macro_def;
    visitor.skip_context.update_with_attrs(&krate.attrs);
    visitor.last_pos = snippet_provider.start_pos();
    visitor.skip_empty_lines(snippet_provider.end_pos());
    visitor.format_separate_mod(module, snippet_provider.end_pos());

    debug_assert_eq!(
        visitor.line_number,
        count_newlines(&visitor.buffer),
        "failed in format_file visitor.buffer:\n {:?}",
        &visitor.buffer
    );

    // For some reason, the source_map does not include terminating
    // newlines so we must add one on for each file. This is sad.
    visitor.buffer.push('\n');

    format_lines(
        &mut visitor.buffer,
        &path,
        &visitor.skipped_range.borrow(),
        config,
        report.clone(),
    );

    // SourceFile's in the SourceMap will always have Unix-style line endings
    // See: https://github.com/rust-lang/rustfmt/issues/3850
    // So we must check the file system to get the original file value in order
    // to detect newline_style conflicts.
    // Otherwise, parse session is around (cfg(not(test))) and newline_style has been
    // left as the default value, then try getting source from the parse session
    // source map instead of hitting the file system.
    let original_text = match original_snippet {
        Some(snippet) => snippet,
        None => std::fs::read_to_string(path.as_path().ok_or_else(|| {
            OperationError::IoError(std::io::Error::from(std::io::ErrorKind::InvalidInput))
        })?)?,
    };
    apply_newline_style(config.newline_style(), &mut visitor.buffer, &original_text);

    if visitor.macro_rewrite_failure {
        report.add_macro_format_failure(path.clone());
    }
    let format_result = FormatResult::success(
        visitor.buffer.to_owned(),
        visitor.skipped_range.borrow().clone(),
        original_text,
        config.newline_style(),
    );
    report.add_format_result(path.clone(), format_result);

    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Timer {
    Disabled,
    Initialized(Instant),
    DoneParsing(Instant, Instant),
    DoneFormatting(Instant, Instant, Instant),
}

impl Timer {
    fn start() -> Timer {
        if cfg!(all(target_arch = "wasm32", not(target_os = "wasi"))) {
            Timer::Disabled
        } else {
            Timer::Initialized(Instant::now())
        }
    }
    fn done_parsing(self) -> Self {
        match self {
            Timer::Disabled => Timer::Disabled,
            Timer::Initialized(init_time) => Timer::DoneParsing(init_time, Instant::now()),
            _ => panic!("Timer can only transition to DoneParsing from Initialized state"),
        }
    }

    fn done_formatting(self) -> Self {
        match self {
            Timer::Disabled => Timer::Disabled,
            Timer::DoneParsing(init_time, parse_time) => {
                Timer::DoneFormatting(init_time, parse_time, Instant::now())
            }
            _ => panic!("Timer can only transition to DoneFormatting from DoneParsing state"),
        }
    }

    /// Returns the time it took to parse the source files in seconds.
    fn get_parse_time(&self) -> f32 {
        match *self {
            Timer::Disabled => panic!("this platform cannot time execution"),
            Timer::DoneParsing(init, parse_time) | Timer::DoneFormatting(init, parse_time, _) => {
                // This should never underflow since `Instant::now()` guarantees monotonicity.
                Self::duration_to_f32(parse_time.duration_since(init))
            }
            Timer::Initialized(..) => unreachable!(),
        }
    }

    /// Returns the time it took to go from the parsed AST to the formatted output. Parsing time is
    /// not included.
    fn get_format_time(&self) -> f32 {
        match *self {
            Timer::Disabled => panic!("this platform cannot time execution"),
            Timer::DoneFormatting(_init, parse_time, format_time) => {
                Self::duration_to_f32(format_time.duration_since(parse_time))
            }
            Timer::DoneParsing(..) | Timer::Initialized(..) => unreachable!(),
        }
    }

    fn duration_to_f32(d: Duration) -> f32 {
        d.as_secs() as f32 + d.subsec_nanos() as f32 / 1_000_000_000f32
    }
}

// Formatting done on a char by char or line by line basis.
// FIXME(#20): other stuff for parity with make tidy.
fn format_lines(
    text: &mut String,
    name: &FileName,
    skipped_range: &[NonFormattedRange],
    config: &Config,
    report: FormatReport,
) {
    let mut formatter = FormatLines::new(name, skipped_range, config);
    if let Some(false) = formatter.check_license(text) {
        report.add_license_failure(name.clone());
    }
    formatter.iterate(text);

    if formatter.newline_count > 1 {
        debug!("track truncate: {} {}", text.len(), formatter.newline_count);
        let line = text.len() - formatter.newline_count + 1;
        text.truncate(line);
    }

    report.append_errors(name.clone(), formatter.errors.into_iter());
}

struct FormatLines<'a> {
    name: &'a FileName,
    skipped_range: &'a [NonFormattedRange],
    last_was_space: bool,
    line_len: usize,
    cur_line: usize,
    newline_count: usize,
    errors: Vec<FormatError>,
    line_buffer: String,
    current_line_contains_string_literal: bool,
    format_line: bool,
    config: &'a Config,
}

impl<'a> FormatLines<'a> {
    fn new(
        name: &'a FileName,
        skipped_range: &'a [NonFormattedRange],
        config: &'a Config,
    ) -> FormatLines<'a> {
        FormatLines {
            name,
            skipped_range,
            last_was_space: false,
            line_len: 0,
            cur_line: 1,
            newline_count: 0,
            errors: vec![],
            line_buffer: String::with_capacity(config.max_width() * 2),
            current_line_contains_string_literal: false,
            format_line: config.file_lines().contains_line(name, 1),
            config,
        }
    }

    fn check_license(&mut self, text: &str) -> Option<bool> {
        self.config
            .license_template
            .as_ref()
            .map(|license_template| license_template.is_match(text))
    }

    // Iterate over the chars in the file map.
    fn iterate(&mut self, text: &str) {
        for (kind, c) in CharClasses::new(text.chars()) {
            if c == '\r' {
                continue;
            }

            if c == '\n' {
                self.new_line(kind);
            } else {
                self.char(c, kind);
            }
        }
    }

    fn new_line(&mut self, kind: FullCodeCharKind) {
        if self.format_line {
            // Check for (and record) trailing whitespace.
            if self.last_was_space {
                if self.should_report_error(kind, &ErrorKind::TrailingWhitespace)
                    && !self.is_skipped_line()
                {
                    self.push_err(ErrorKind::TrailingWhitespace);
                }
                self.line_len -= 1;
            }

            // Check for any line width errors we couldn't correct.
            let error_kind = ErrorKind::LineOverflow(self.line_len, self.config.max_width());
            if self.line_len > self.config.max_width()
                && !self.is_skipped_line()
                && self.should_report_error(kind, &error_kind)
            {
                self.push_err(error_kind);
            }
        }

        self.line_len = 0;
        self.cur_line += 1;
        self.format_line = self
            .config
            .file_lines()
            .contains_line(self.name, self.cur_line);
        self.newline_count += 1;
        self.last_was_space = false;
        self.line_buffer.clear();
        self.current_line_contains_string_literal = false;
    }

    fn char(&mut self, c: char, kind: FullCodeCharKind) {
        self.newline_count = 0;
        self.line_len += if c == '\t' {
            self.config.tab_spaces()
        } else {
            1
        };
        self.last_was_space = c.is_whitespace();
        self.line_buffer.push(c);
        if kind.is_string() {
            self.current_line_contains_string_literal = true;
        }
    }

    fn push_err(&mut self, kind: ErrorKind) {
        self.errors.push(FormatError::new(
            kind,
            self.cur_line,
            self.line_buffer.clone(),
        ));
    }

    fn should_report_error(&self, char_kind: FullCodeCharKind, error_kind: &ErrorKind) -> bool {
        let allow_error_report =
            if char_kind.is_comment() || self.current_line_contains_string_literal {
                self.config.error_on_unformatted()
            } else {
                true
            };

        match error_kind {
            ErrorKind::LineOverflow(..) => {
                self.config.error_on_line_overflow() && allow_error_report
            }
            ErrorKind::TrailingWhitespace => allow_error_report,
            _ => true,
        }
    }

    /// Returns `true` if the line with the given line number was skipped by `#[rustfmt::skip]`.
    fn is_skipped_line(&self) -> bool {
        self.skipped_range
            .iter()
            .any(|range| range.contains(self.cur_line))
    }
}

fn should_emit_verbose<F>(forbid_verbose_output: bool, verbosity: Verbosity, f: F)
where
    F: Fn(),
{
    if verbosity == Verbosity::Verbose && !forbid_verbose_output {
        f();
    }
}

/// Result of formatting a snippet of code along with ranges of lines that didn't get formatted,
/// i.e., that got returned as they were originally.
#[derive(Debug, Clone, Default)]
pub(crate) struct FormattedSnippet {
    snippet: String,
    non_formatted_ranges: Vec<NonFormattedRange>,
}

impl FormattedSnippet {
    /// In case the snippet needed to be wrapped in a function, this shifts down the ranges of
    /// non-formatted code.
    fn unwrap_code_block(&mut self) {
        self.non_formatted_ranges.iter_mut().for_each(|range| {
            *range = range.shift_up();
        });
    }

    /// Returns `true` if the line n did not get formatted.
    pub(crate) fn is_line_non_formatted(&self, n: usize) -> bool {
        self.non_formatted_ranges
            .iter()
            .any(|range| range.contains(n))
    }
}

impl AsRef<str> for FormattedSnippet {
    fn as_ref(&self) -> &str {
        self.snippet.as_str()
    }
}

impl Input {
    fn file_name(&self) -> FileName {
        match *self {
            Input::File(ref file) => FileName::Real(file.clone()),
            Input::Text(..) => FileName::Stdin,
        }
    }

    fn to_directory_ownership(&self, recursive: bool) -> Option<DirectoryOwnership> {
        match self {
            // On recursive mode, we assume that input is the root file.
            Input::File(..) if recursive => None,
            Input::File(ref file) => {
                // If there exists a directory with the same name as an input,
                // then the input should be parsed as a sub module.
                let file_stem = file.file_stem()?;
                if file.parent()?.to_path_buf().join(file_stem).is_dir() {
                    Some(DirectoryOwnership::Owned {
                        relative: file_stem.to_str().map(symbol::Ident::from_str),
                    })
                } else {
                    None
                }
            }
            Input::Text(..) => None,
        }
    }
}
