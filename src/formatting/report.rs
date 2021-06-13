use std::cell::{Ref, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::formatting::FormattedSnippet;
use crate::result::{ErrorKind, FormatError};
use crate::Config;
use crate::FileName;
use crate::NewlineStyle;

/// Reports on any issues that occurred during a run of Rustfmt.
///
/// Can be reported to the user using the `Display` impl on [`FormatReportFormatter`].
#[derive(Debug, Clone)]
pub struct FormatReport {
    format_result: Rc<RefCell<BTreeMap<FileName, FormatResult>>>,
    ignored_files: Rc<RefCell<BTreeSet<FileName>>>,
}

/// The result of formatting, including the formatted text and various
/// errors and warning arose while formatting.
#[derive(Debug, Clone, Default)]
pub struct FormatResult {
    original_snippet: String,
    formatted_snippet: FormattedSnippet,
    format_errors: HashSet<FormatError>,
    newline_style: NewlineStyle,
}

/// The inclusive range of the input which was not formatted, represented by a pair of line numbers.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct NonFormattedRange {
    lo: usize,
    hi: usize,
}

impl FormatResult {
    pub(crate) fn success(
        snippet: String,
        non_formatted_ranges: Vec<NonFormattedRange>,
        original_snippet: String,
        newline_style: NewlineStyle,
    ) -> Self {
        let formatted_snippet = FormattedSnippet {
            snippet,
            non_formatted_ranges,
        };
        FormatResult {
            original_snippet,
            formatted_snippet,
            format_errors: HashSet::new(),
            newline_style,
        }
    }

    pub(crate) fn all_errors(&self) -> impl Iterator<Item = &FormatError> {
        self.format_errors.iter()
    }

    pub(crate) fn errors_excluding_macro(&self) -> impl Iterator<Item = &FormatError> {
        self.format_errors
            .iter()
            .filter(|e| e.kind() != ErrorKind::MacroFormatError)
    }

    /// Return the newline style used to format the result.
    pub fn newline_style(&self) -> NewlineStyle {
        self.newline_style
    }

    pub fn original_text(&self) -> &str {
        &self.original_snippet
    }

    pub fn formatted_text(&self) -> &str {
        &self.formatted_snippet.snippet
    }

    pub(crate) fn formatted_snippet(&self) -> &FormattedSnippet {
        &self.formatted_snippet
    }

    pub(crate) fn has_error_kind(&self, kind: ErrorKind) -> bool {
        self.all_errors().any(|e| e.kind() == kind)
    }

    pub(crate) fn has_any_matching_errors<F>(&self, error_matcher: F) -> bool
    where
        F: FnMut(&FormatError) -> bool,
    {
        self.all_errors().any(error_matcher)
    }
}

impl FormatReport {
    pub fn new() -> FormatReport {
        FormatReport {
            format_result: Rc::new(RefCell::new(BTreeMap::new())),
            ignored_files: Rc::new(RefCell::new(BTreeSet::new())),
        }
    }

    /// Returns the result of formatting the given input, including the formatted text and
    /// various warnings and errors encountered during formatting.
    //
    // NOTE: Avoid using this method internally: use `format_result_as_rc` instead.
    pub fn format_result(&self) -> impl Iterator<Item = (&FileName, &FormatResult)> {
        Ref::leak(RefCell::borrow(&self.format_result)).iter()
    }

    pub(crate) fn format_result_as_rc(&self) -> Rc<RefCell<BTreeMap<FileName, FormatResult>>> {
        Rc::clone(&self.format_result)
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.format_result
            .borrow_mut()
            .append(&mut other.format_result.borrow_mut());
        self.ignored_files
            .borrow_mut()
            .append(&mut other.ignored_files.borrow_mut());
    }

    pub(crate) fn add_ignored_file(&self, file_name: FileName) {
        self.ignored_files.borrow_mut().insert(file_name);
    }

    pub(crate) fn add_format_result(&self, file_name: FileName, format_result: FormatResult) {
        let mut format_results = self.format_result.borrow_mut();
        let mut original_format_result = format_results.entry(file_name).or_default();
        original_format_result.formatted_snippet = format_result.formatted_snippet;
        original_format_result
            .format_errors
            .extend(format_result.format_errors);
        original_format_result.original_snippet = format_result.original_snippet;
    }

    pub(crate) fn append_errors(&self, f: FileName, errors: impl Iterator<Item = FormatError>) {
        let mut format_result = self.format_result.borrow_mut();
        let format_errors = &mut format_result.entry(f).or_default().format_errors;
        for err in errors {
            format_errors.insert(err);
        }
    }

    pub(crate) fn add_macro_format_failure(&self, file_name: FileName) {
        self.add_format_error(
            file_name,
            FormatError::err_without_line_info(ErrorKind::MacroFormatError),
        );
    }

    pub(crate) fn add_license_failure(&self, file_name: FileName) {
        self.add_format_error(
            file_name,
            FormatError::err_without_line_info(ErrorKind::LicenseCheck),
        );
    }

    pub(crate) fn add_format_error(&self, file_name: FileName, format_error: FormatError) {
        self.format_result
            .borrow_mut()
            .entry(file_name)
            .or_default()
            .format_errors
            .insert(format_error);
    }

    fn has_any_matching_format_result<F>(&self, format_result_matcher: F) -> bool
    where
        F: FnMut((&FileName, &FormatResult)) -> bool,
    {
        RefCell::borrow(&self.format_result)
            .iter()
            .any(format_result_matcher)
    }

    fn has_error_kind(&self, kind: ErrorKind) -> bool {
        self.has_any_matching_format_result(|(_, format_result)| format_result.has_error_kind(kind))
    }

    pub fn has_deprecated_attribute_errors(&self) -> bool {
        self.has_error_kind(ErrorKind::DeprecatedAttr)
    }

    pub fn has_invalid_rustfmt_attribute_errors(&self) -> bool {
        self.has_error_kind(ErrorKind::BadAttr)
    }

    pub fn has_attribute_errors(&self) -> bool {
        self.has_any_matching_format_result(|(_, format_result)| {
            format_result.has_any_matching_errors(|e| {
                matches!(e.kind(), ErrorKind::BadAttr | ErrorKind::DeprecatedAttr)
            })
        })
    }

    pub fn has_failing_errors(&self, file_config_map: HashMap<FileName, &Config>) -> bool {
        self.has_any_matching_format_result(|(file_name, format_result)| {
            format_result.has_any_matching_errors(|e| match e.kind() {
                ErrorKind::BadAttr | ErrorKind::DeprecatedAttr => true,
                ErrorKind::LicenseCheck => {
                    if let Some(config) = file_config_map.get(file_name) {
                        if config.was_set().license_template_path() {
                            return true;
                        }
                    }
                    false
                }
                ErrorKind::LineOverflow(..) => {
                    if let Some(config) = file_config_map.get(file_name) {
                        if config.error_on_line_overflow() {
                            return true;
                        }
                    }
                    false
                }
                ErrorKind::TrailingWhitespace => {
                    if let Some(config) = file_config_map.get(file_name) {
                        if config.error_on_unformatted() {
                            return true;
                        }
                    }
                    false
                }
                _ => false,
            })
        })
    }

    pub fn has_errors(&self) -> bool {
        RefCell::borrow(&self.format_result)
            .iter()
            .any(|(_, format_result)| format_result.errors_excluding_macro().count() > 0)
    }

    pub(crate) fn warning_count(&self) -> usize {
        RefCell::borrow(&self.format_result)
            .iter()
            .map(|(_, format_result)| format_result.errors_excluding_macro().count())
            .sum()
    }
}

impl NonFormattedRange {
    pub(crate) fn new(lo: usize, hi: usize) -> NonFormattedRange {
        NonFormattedRange { lo, hi }
    }

    pub(crate) fn shift_up(self) -> NonFormattedRange {
        NonFormattedRange {
            lo: self.lo - 1,
            hi: self.hi - 1,
        }
    }

    pub(crate) fn contains(&self, line: usize) -> bool {
        self.lo <= line && line <= self.hi
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::formatting::utils::format_snippet;

    #[cfg(test)]
    mod has_failing_errors {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn false_with_only_macro() {
            let file_name = FileName::Real(PathBuf::from("foo/bar.rs"));
            let report = FormatReport::new();
            report.add_format_error(
                file_name.clone(),
                FormatError::new(ErrorKind::MacroFormatError, 2, String::new()),
            );
            assert!(
                !report.has_failing_errors(
                    vec![(file_name, &Config::default())].into_iter().collect()
                )
            );
        }

        #[test]
        fn true_with_bad_attr() {
            let file_name = FileName::Real(PathBuf::from("bar/baz.rs"));
            let report = FormatReport::new();
            report.add_format_error(
                file_name.clone(),
                FormatError::new(ErrorKind::BadAttr, 2, String::new()),
            );
            assert!(
                report.has_failing_errors(
                    vec![(file_name, &Config::default())].into_iter().collect()
                )
            );
        }

        #[test]
        fn true_with_deprecated_attr() {
            let file_name = FileName::Real(PathBuf::from("baz/qux.rs"));
            let report = FormatReport::new();
            report.add_format_error(
                file_name.clone(),
                FormatError::new(ErrorKind::DeprecatedAttr, 2, String::new()),
            );
            assert!(
                report.has_failing_errors(
                    vec![(file_name, &Config::default())].into_iter().collect()
                )
            );
        }

        #[test]
        fn false_with_license_check_and_config_disabled() {
            let file_name = FileName::Real(PathBuf::from("foo.rs"));
            let bar_file_name = FileName::Real(PathBuf::from("bar.rs"));
            let mut license_config = Config::default();
            license_config
                .set()
                .license_template_path(String::from("template.txt"));
            let report = FormatReport::new();
            report.add_format_error(
                bar_file_name.clone(),
                FormatError::new(ErrorKind::LicenseCheck, 2, String::new()),
            );
            assert!(
                !report.has_failing_errors(
                    vec![
                        (file_name, &license_config),
                        (bar_file_name, &Config::default()),
                    ]
                    .into_iter()
                    .collect(),
                )
            );
        }

        #[test]
        fn true_with_license_check_and_config_enabled() {
            let file_name = FileName::Real(PathBuf::from("foo.rs"));
            let report = FormatReport::new();
            let mut config = Config::default();
            config
                .set()
                .license_template_path(String::from("license.txt"));
            report.add_license_failure(file_name.clone());
            assert!(report.has_failing_errors(vec![(file_name, &config)].into_iter().collect()));
        }

        #[test]
        fn false_with_line_overflow_and_config_disabled() {
            let file_name = FileName::Real(PathBuf::from("short_enough.rs"));
            let overflow_file_name = FileName::Real(PathBuf::from("too_long.rs"));
            let mut overflow_config = Config::default();
            overflow_config.set().error_on_line_overflow(true);
            let report = FormatReport::new();
            report.add_license_failure(file_name.clone());
            assert!(
                !report.has_failing_errors(
                    vec![
                        (file_name, &overflow_config),
                        (overflow_file_name, &Config::default()),
                    ]
                    .into_iter()
                    .collect(),
                )
            );
        }

        #[test]
        fn true_with_line_overflow_and_config_enabled() {
            let file_name = FileName::Real(PathBuf::from("overflowed.rs"));
            let report = FormatReport::new();
            let mut config = Config::default();
            config.set().error_on_line_overflow(true);
            report.add_format_error(
                file_name.clone(),
                FormatError::new(ErrorKind::LineOverflow(100, 103), 2, String::new()),
            );
            assert!(report.has_failing_errors(vec![(file_name, &config)].into_iter().collect()));
        }

        #[test]
        fn false_with_trailing_whitespace_and_config_disabled() {
            let file_name = FileName::Real(PathBuf::from("trimmed.rs"));
            let trailing_file_name = FileName::Real(PathBuf::from("trailing_whitespace.rs"));
            let mut trailing_config = Config::default();
            trailing_config.set().error_on_unformatted(true);
            let report = FormatReport::new();
            report.add_format_error(
                trailing_file_name.clone(),
                FormatError::new(ErrorKind::TrailingWhitespace, 3, String::new()),
            );
            assert!(
                !report.has_failing_errors(
                    vec![
                        (file_name, &trailing_config),
                        (trailing_file_name, &Config::default()),
                    ]
                    .into_iter()
                    .collect(),
                )
            );
        }

        #[test]
        fn true_with_trailing_whitespace_and_config_enabled() {
            let file_name = FileName::Real(PathBuf::from("trailing_whitespace.rs"));
            let report = FormatReport::new();
            let mut config = Config::default();
            config.set().error_on_unformatted(true);
            report.add_format_error(
                file_name.clone(),
                FormatError::new(ErrorKind::TrailingWhitespace, 42, String::new()),
            );
            assert!(report.has_failing_errors(vec![(file_name, &config)].into_iter().collect()));
        }
    }

    #[test]
    fn test_skipped_ranges() {
        #[rustfmt::skip]
        let snippet = "// Impl - original code from issue #4706
            impl      Foo          {
            #[rustfmt::skip] // Lines 4-7
            fn foo() {
            Bar     
            //     ^ there is whitespace here
            }

            fn bar() {
            Qux // asdf
            }
            }

// Impl - all skipped lines end with white spaces
            #[rustfmt::skip] // Lines 15-22
            /// DOC1   
            /// DOC2   
            impl      Foo1          {     
            fn foo1() {
            Bar1     
            }   
            }     

            impl      Foo2          {
            #[rustfmt::skip] // Lines 26-30
            /// DOC1   
            /// DOC2   
            fn foo2() {    
            Bar2     
            }       
            }

            impl      Foo3          {
            fn foo3() {
            #[rustfmt::skip] // Lines 36-38
            /// DOC1   
            /// DOC2        
            Bar2     
            }  
            }

// fn - all skipped lines end with white spaces            
            #[rustfmt::skip] // Lines 43-48
            /// DOC1   
            /// DOC2   
            fn foo2() {    
            Bar2     
            }       

            fn foo3() {
            #[rustfmt::skip] // Lines 52-54
            /// DOC1   
            /// DOC2        
            Bar2     
            }

// Trait - all skipped lines end with white spaces

            // All skipped lines end with white spaces
            #[rustfmt::skip] // Lines 60-66
            #[allow(non_snake_case)]    
            trait Animal1 {    
            fn new(name: &'static str) -> Self;    

            fn talk(&self) {}     
            }    

            // All skipped lines end with white spaces
            #[allow(non_snake_case)]
            trait Animal1 {
            #[rustfmt::skip] // Lines 72-72
            fn new(name: &'static str) -> Self;    

            fn talk(&self) {}    
            }

            // Internal skipped line
            #[allow(non_snake_case)]
            trait Animal3 {
            fn new(name: &'static str) -> Self;

            fn talk(&self) {
            #[rustfmt::skip] // Lines 84-84
            let x = 1;    
            //          ^ there is whitespace here
            }
            }

// Macro - all skipped lines end with white spaces.
            #[rustfmt::skip] // Lines  90-93
            macro_rules! my_macro1 {    
            () => {};    

        }";

        let mut actual: Vec<NonFormattedRange> = vec![];
        #[rustfmt::skip]
        let expected: Vec<NonFormattedRange> = vec![NonFormattedRange { lo: 4, hi: 7 }, NonFormattedRange { lo: 15, hi: 22 }, NonFormattedRange { lo: 26, hi: 30 }, NonFormattedRange { lo: 36, hi: 38 }, NonFormattedRange { lo: 43, hi: 48 }, NonFormattedRange { lo: 52, hi: 54 }, NonFormattedRange { lo: 60, hi: 66 }, NonFormattedRange { lo: 72, hi: 72 }, NonFormattedRange { lo: 84, hi: 84 }, NonFormattedRange { lo: 90, hi: 94 }];

        let output = format_snippet(snippet, &Config::default(), false);

        let output_ok = output.map_or(false, |o| {
            actual = o.non_formatted_ranges;
            (actual.len() == expected.len())
                && actual.iter().zip(expected.clone()).all(|(a, b)| *a == b)
        });

        assert!(output_ok, "\nexpect={:?},\nactual={:?}", expected, actual);
    }
}
