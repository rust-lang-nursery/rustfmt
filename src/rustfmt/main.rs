#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::env;
use std::fmt;
use std::io::{self, stdin, stdout, Error as IoError, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{format_err, Result};
use structopt::StructOpt;
use thiserror::Error;

use rustfmt_nightly::NewlineStyle;

use rustfmt_nightly::{
    emitter::{emit_format_report, EmitMode, EmitterConfig, Verbosity},
    format_inputs, load_config, CliOptions, Config, Edition, FileLines, FileName,
    FormatReportFormatterBuilder, Input, OperationSetting,
};

fn main() {
    env_logger::Builder::from_env("RUSTFMT_LOG").init();

    let opt: Opt = Opt::from_args();

    let exit_code = match execute(opt) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}", e.to_string());
            1
        }
    };
    // Make sure standard output is flushed before we exit.
    std::io::stdout().flush().unwrap();

    // Exit with given exit code.
    //
    // NOTE: this immediately terminates the process without doing any cleanup,
    // so make sure to finish all necessary cleanup before this is called.
    std::process::exit(exit_code);
}

/// Format Rust code
#[derive(Debug, StructOpt, Clone)]
#[structopt(
    name = "rustfmt",
    version = include_str!(concat!(env!("OUT_DIR"),"/version-info.txt")),
    about = r#"Format Rust code

Rustfmt runs on a set of files or stdin. When invoked without any file arguments, rustfmt will
read code from stdin.

Please visit https://rust-lang.github.io/rustfmt to see all rustfmt configuration options.

EXAMPLES

    cat lib.rs | rustfmt
        Feed the contents of "lib.rs" to rustfmt via stdin

    rustfmt lib.rs main.rs
        Run rustfmt over "lib.rs" and "main.rs", formatting in-place.

    rustfmt --emit=stdout lib.rs main.rs
        Run rustfmt over "lib.rs" and "main.rs", writing to stdout (rather than in-place)

    rustfmt --config-path=rustfmt.toml --print-config=current
        Print the resolved rustfmt configuration formed by rustfmt.toml
"#)]
struct Opt {
    /// Run in 'check' mode.
    ///
    /// Exits with 0 if input is formatted correctly.
    /// Exits with 1 and prints a diff if formatting is required.
    #[structopt(short, long)]
    check: bool,
    /// Specify the format of rustfmt's output.
    #[cfg_attr(nightly, structopt(long, name = "files|stdout|checkstyle|json"))]
    #[cfg_attr(not(nightly), structopt(long, name = "files|stdout"))]
    emit: Option<Emit>,
    /// A path to the configuration file.
    #[structopt(long = "config-path", parse(from_os_str))]
    config_path: Option<PathBuf>,
    /// Rust compiler edition
    ///
    /// Specify which edition of the compiler to use when formatting code. This behaves identically
    /// to the "edition" configuration option.
    #[structopt(long, name = "2015|2018")]
    edition: Option<Edition>,
    /// Print configuration options.
    ///
    /// `default` will print the default configuration options. `current` will print the
    /// current configuration options. `minimal` will print the minimal subset of the
    /// current configuration options that have non-default values.
    #[structopt(long = "print-config", name = "default|current|minimal")]
    print_config: Option<PrintConfig>,
    /// Prints the names of files with diff.
    #[structopt(short = "l", long = "files-with-diff")]
    files_with_diff: bool,
    /// Set options from command line.
    ///
    /// Set configuration options via command line by specifying a list of key-value pairs
    /// separated by commas (e.g., rustfmt --config=max_width=100,imports_granularity=Crate).
    /// These settings precedes any other settings specified in configuration files.
    #[structopt(long = "config")]
    inline_config: Option<Vec<InlineConfig>>,
    /// Recursively format submodules.
    ///
    /// Format all encountered modules recursively regardless of whether the modules
    /// are defined inline or in another file.
    #[structopt(short, long)]
    recursive: bool,
    /// Print no output.
    #[structopt(short, long)]
    quiet: bool,
    /// Print verbose output.
    #[structopt(short, long)]
    verbose: bool,
    /// Continue with reformatting even if there are errors.
    #[structopt(short, long)]
    force: bool,

    // Nightly-only options.
    /// Limit formatting to specified ranges.
    ///
    /// If you want to restrict reformatting to specific sets of lines, you can
    /// use the `--file-lines` option. Its argument is a JSON array of objects
    /// with `file` and `range` properties, where `file` is a file name, and
    /// `range` is an array representing a range of lines like `[7,13]`. Ranges
    /// are 1-based and inclusive of both end points. Specifying an empty array
    /// will result in no files being formatted. For example,
    ///
    /// ```
    /// rustfmt --file-lines '[
    ///    {{\"file\":\"src/lib.rs\",\"range\":[7,13]}},
    ///    {{\"file\":\"src/lib.rs\",\"range\":[21,29]}},
    ///    {{\"file\":\"src/foo.rs\",\"range\":[10,11]}},
    ///    {{\"file\":\"src/foo.rs\",\"range\":[15,15]}}]'
    /// ```
    ///
    /// would format lines `7-13` and `21-29` of `src/lib.rs`, and lines `10-11`,
    /// and `15` of `src/foo.rs`. No other files would be formatted, even if they
    /// are included as out of line modules from `src/lib.rs`.
    #[cfg_attr(nightly, structopt(long = "file-lines", default_value = "null"))]
    #[cfg_attr(not(nightly), structopt(skip))]
    file_lines: FileLines,

    /// Error if unable to get comments or string literals within max_width,
    /// or they are left with trailing whitespaces (unstable).
    #[cfg_attr(nightly, structopt(long = "error-on-unformatted"))]
    #[cfg_attr(not(nightly), structopt(skip))]
    error_on_unformatted: bool,

    // Positional arguments.
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,
}

impl Opt {
    fn verbosity(&self) -> Verbosity {
        if self.quiet || self.files.is_empty() {
            Verbosity::Quiet
        } else if self.verbose {
            Verbosity::Verbose
        } else {
            Verbosity::Normal
        }
    }

    fn emitter_config(
        &self,
        default_emit_mode: EmitMode,
        newline_style: NewlineStyle,
    ) -> EmitterConfig {
        let emit_mode = if self.check {
            EmitMode::Diff
        } else {
            self.emit.map_or(default_emit_mode, Emit::to_emit_mode)
        };
        EmitterConfig {
            emit_mode,
            verbosity: self.verbosity(),
            print_filename: self.files_with_diff,
            newline_style: newline_style,
            ..EmitterConfig::default()
        }
    }
}

#[derive(Debug, Clone)]
struct InlineConfig(HashMap<String, String>, bool /* is help */);

impl InlineConfig {
    fn is_help(&self) -> bool {
        self.1
    }
}

impl FromStr for InlineConfig {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim() == "help" {
            return Ok(InlineConfig(HashMap::default(), true));
        }

        s.split(',')
            .map(
                |key_val| match key_val.char_indices().find(|(_, ch)| *ch == '=') {
                    Some((middle, _)) => {
                        let (key, val) = (&key_val[..middle], &key_val[middle + 1..]);
                        if !Config::is_valid_key_val(key, val) {
                            Err(format_err!("invalid key=val pair: `{}`", key_val))
                        } else {
                            Ok((key.to_string(), val.to_string()))
                        }
                    }

                    None => Err(format_err!(
                        "--config expects comma-separated list of key=val pairs, found `{}`",
                        key_val
                    )),
                },
            )
            .collect::<Result<HashMap<_, _>, _>>()
            .map(|map| InlineConfig(map, false))
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum PrintConfig {
    Default,
    Minimal,
    Current,
}

impl FromStr for PrintConfig {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(PrintConfig::Default),
            "minimal" => Ok(PrintConfig::Minimal),
            "current" => Ok(PrintConfig::Current),
            _ => Err(format!(
                "expected one of [current,default,minimal], found `{}`",
                s
            )),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Emit {
    Files,
    Stdout,
    Checkstyle,
    Json,
}

impl Emit {
    fn to_emit_mode(self) -> EmitMode {
        match self {
            Emit::Files => EmitMode::Files,
            Emit::Json => EmitMode::Json,
            Emit::Checkstyle => EmitMode::Checkstyle,
            Emit::Stdout => EmitMode::Stdout,
        }
    }
}

impl fmt::Display for Emit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Emit::Files => f.write_str("files"),
            Emit::Stdout => f.write_str("stdout"),
            Emit::Checkstyle => f.write_str("checkstyle"),
            Emit::Json => f.write_str("json"),
        }
    }
}

impl FromStr for Emit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "files" => Ok(Emit::Files),
            "stdout" => Ok(Emit::Stdout),
            "checkstyle" => Ok(Emit::Checkstyle),
            "json" => Ok(Emit::Json),
            _ => Err(format!("unknown --emit mode: {}", s)),
        }
    }
}

/// Rustfmt command line option errors.
#[derive(Error, Debug)]
enum OptError {
    /// Attempt to use --quiet and --verbose at once.
    #[error("--quiet and --verbose cannot be used at once.")]
    QuietAndVerbose,
    /// Attempt to use --emit and --check at once.
    #[error("--emit and --check cannot be used at once.")]
    EmitAndCheck,
    /// Attempt to use --emit with a mode which is not currently
    /// supported with standard input.
    #[error("Emit mode {0} not supported with standard output.")]
    StdinBadEmit(Emit),
}

impl Opt {
    fn canonicalize(&mut self) {
        for f in &mut self.files {
            if let Ok(canonical_path) = dunce::canonicalize(&f) {
                *f = canonical_path;
            }
        }
    }

    fn verify(&self) -> Result<(), OptError> {
        if self.quiet && self.verbose {
            return Err(OptError::QuietAndVerbose);
        }

        if self.check && self.emit.is_some() {
            return Err(OptError::EmitAndCheck);
        }

        if self.files.is_empty() {
            match self.emit {
                // Emit modes which work with standard input
                // None means default, which is Stdout.
                None | Some(Emit::Stdout) | Some(Emit::Checkstyle) | Some(Emit::Json) => {}
                Some(emit_mode) => {
                    return Err(OptError::StdinBadEmit(emit_mode));
                }
            }
        }

        Ok(())
    }
}

/// Rustfmt operations errors.
#[derive(Error, Debug)]
pub enum OperationError {
    /// An unknown help topic was requested.
    #[error("Unknown help topic: `{0}`.")]
    UnknownHelpTopic(String),
    /// An unknown print-config option was requested.
    #[error("Unknown print-config option: `{0}`.")]
    UnknownPrintConfigTopic(String),
    /// Attempt to generate a minimal config from standard input.
    #[error("The `--print-config=minimal` option doesn't work with standard input.")]
    MinimalPathWithStdin,
    /// An io error during reading or writing.
    #[error("{0}")]
    IoError(IoError),
}

impl CliOptions for Opt {
    fn apply_to(&self, config: &mut Config) {
        config.set().file_lines(self.file_lines.clone());
        if self.error_on_unformatted {
            config.set().error_on_unformatted(true);
        }
        if let Some(ref edition) = self.edition {
            config.set().edition(*edition);
        }
        if let Some(ref inline_configs) = self.inline_config {
            for inline_config in inline_configs {
                for (k, v) in &inline_config.0 {
                    config.override_value(k, v);
                }
            }
        }
    }

    fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }
}

// Returned i32 is an exit code
fn execute(mut opt: Opt) -> Result<i32> {
    opt.verify()?;

    if opt.inline_config.as_ref().map_or(false, |inline_configs| {
        inline_configs.iter().any(InlineConfig::is_help)
    }) {
        Config::print_docs(&mut stdout(), cfg!(nightly));
        return Ok(0);
    }

    opt.canonicalize();

    match opt.print_config {
        Some(PrintConfig::Default) => print_default_config(),
        Some(PrintConfig::Minimal) => print_config(&opt, PrintConfig::Minimal),
        Some(PrintConfig::Current) => print_config(&opt, PrintConfig::Current),
        None => format(opt),
    }
}

fn print_default_config() -> Result<i32> {
    let toml = Config::default().all_options().to_toml()?;
    io::stdout().write_all(toml.as_bytes())?;
    Ok(0)
}

fn print_config(opt: &Opt, print_config: PrintConfig) -> Result<i32> {
    let (config, config_path) = load_config(env::current_dir().ok().as_deref(), Some(opt))?;
    let actual_config =
        FileConfigPairIter::new(&opt, config_path.is_some()).find_map(|pair| match pair.config {
            FileConfig::Local(config, Some(_)) => Some(config),
            _ => None,
        });
    let used_config = actual_config.unwrap_or(config);
    let toml = if print_config == PrintConfig::Minimal {
        used_config.used_options().to_toml()?
    } else {
        used_config.all_options().to_toml()?
    };
    io::stdout().write_all(toml.as_bytes())?;
    Ok(0)
}

fn format_string(input: String, opt: Opt) -> Result<i32> {
    // try to read config from local directory
    let (mut config, _) = load_config(Some(Path::new(".")), Some(&opt))?;

    // parse file_lines
    config.set().file_lines(opt.file_lines.clone());
    for f in config.file_lines().files() {
        match *f {
            FileName::Stdin => {}
            _ => eprintln!("Warning: Extra file listed in file_lines option '{}'", f),
        }
    }

    let out = &mut stdout();
    let setting = OperationSetting {
        recursive: opt.recursive,
        verbosity: Verbosity::Quiet,
    };
    let report = rustfmt_nightly::format(Input::Text(input), &config, setting)?;

    if report.has_errors() {
        eprintln!(
            "{}",
            FormatReportFormatterBuilder::new(&report)
                .enable_colors(true)
                .build()
        );

        if !opt.force
            && report.has_failing_errors(vec![(FileName::Stdin, &config)].into_iter().collect())
        {
            return Ok(1);
        }
    }

    let has_diff = emit_format_report(
        report,
        out,
        opt.emitter_config(EmitMode::Stdout, config.newline_style()),
    )?;
    Ok(if opt.check && has_diff { 1 } else { 0 })
}

enum FileConfig {
    Default,
    Local(Config, Option<Vec<PathBuf>>),
}

struct FileConfigPair<'a> {
    file: &'a Path,
    config: FileConfig,
}

struct FileConfigPairIter<'a> {
    has_config_from_commandline: bool,
    files: std::slice::Iter<'a, PathBuf>,
    opt: &'a Opt,
}

impl<'a> FileConfigPairIter<'a> {
    fn new(opt: &'a Opt, has_config_from_commandline: bool) -> Self {
        FileConfigPairIter {
            has_config_from_commandline,
            files: opt.files.iter(),
            opt,
        }
    }
}

impl<'a> Iterator for FileConfigPairIter<'a> {
    type Item = FileConfigPair<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = self.files.next()?;
        let config = if self.has_config_from_commandline {
            FileConfig::Default
        } else {
            let (local_config, config_paths) =
                load_config(Some(file.parent()?), Some(self.opt)).ok()?;
            FileConfig::Local(local_config, config_paths)
        };

        Some(FileConfigPair { file, config })
    }
}

fn format(opt: Opt) -> Result<i32> {
    if opt.files.is_empty() {
        let mut buf = String::new();
        stdin().read_to_string(&mut buf)?;
        return format_string(buf, opt);
    }

    if let Some(file) = opt.files.iter().find(|f| !f.exists()) {
        return Err(format_err!(
            "Error: file `{}` does not exist",
            file.display()
        ));
    }
    if let Some(dir) = opt.files.iter().find(|f| f.is_dir()) {
        return Err(format_err!("Error: `{}` is a directory", dir.display()));
    }

    let (default_config, config_paths) = load_config(None, Some(&opt))?;

    if opt.verbose {
        if let Some(paths) = config_paths.as_ref() {
            println!(
                "Using rustfmt config file(s) {}",
                paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
    }

    let setting = OperationSetting {
        recursive: opt.recursive,
        verbosity: opt.verbosity(),
    };

    let inputs = FileConfigPairIter::new(&opt, config_paths.is_some()).collect::<Vec<_>>();
    let format_report = format_inputs(
        inputs.iter().map(|p| {
            (
                Input::File(p.file.to_path_buf()),
                if let FileConfig::Local(ref config, _) = p.config {
                    config
                } else {
                    &default_config
                },
            )
        }),
        setting,
    )?;

    let print_formatting_errors = || {
        eprintln!(
            "{}",
            FormatReportFormatterBuilder::new(&format_report)
                .enable_colors(true)
                .build()
        );
    };

    match (format_report.has_errors(), opt.force) {
        (false, _) => {}
        (true, true) => print_formatting_errors(),
        (true, false) => {
            print_formatting_errors();
            let file_config_map = inputs
                .iter()
                .map(|p| {
                    (
                        FileName::Real(p.file.to_path_buf()),
                        if let FileConfig::Local(ref config, _) = p.config {
                            config
                        } else {
                            &default_config
                        },
                    )
                })
                .collect();

            if format_report.has_failing_errors(file_config_map) {
                return Ok(1);
            }
        }
    }

    let has_diff = emit_format_report(
        format_report,
        &mut stdout(),
        opt.emitter_config(EmitMode::Files, default_config.newline_style()),
    )?;

    Ok(if opt.check && has_diff { 1 } else { 0 })
}

#[cfg(test)]
mod test {
    use super::*;
    use std::process::{Command, Stdio};

    fn init_log() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    struct TempFile {
        path: PathBuf,
    }

    fn assert_temp_file_contents(expected_contents: &[u8], actual_file: TempFile) {
        assert_eq!(
            expected_contents,
            std::fs::read_to_string(&actual_file.path)
                .expect("couldn't read temp file")
                .as_bytes(),
        );
    }

    fn make_temp_file_with_contents(file_name: &'static str, content: &[u8]) -> TempFile {
        use std::env::var;
        use std::fs::File;

        // Used in the Rust build system.
        let target_dir = var("RUSTFMT_TEST_DIR").unwrap_or_else(|_| ".".to_owned());
        let path = Path::new(&target_dir).join(file_name);

        let mut file = File::create(&path).expect("couldn't create temp file");
        file.write_all(content).expect("couldn't write temp file");
        TempFile { path }
    }

    fn make_temp_file(file_name: &'static str) -> TempFile {
        make_temp_file_with_contents(file_name, b"fn main() {}\n")
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            use std::fs::remove_file;
            remove_file(&self.path).expect("couldn't delete temp file");
        }
    }

    fn rustfmt() -> &'static Path {
        lazy_static! {
            static ref RUSTFMT_PATH: PathBuf = {
                let mut me = env::current_exe().expect("failed to get current executable");
                // Chop of the test name.
                me.pop();
                // Chop off `deps`.
                me.pop();

                // If we run `cargo test --release`, we might only have a release build.
                if cfg!(release) {
                    // `../release/`
                    me.pop();
                    me.push("release");
                }
                me.push("rustfmt");
                assert!(
                    me.is_file() || me.with_extension("exe").is_file(),
                    "{}",
                    if cfg!(release) {
                        "no rustfmt bin, try running `cargo build --release` before testing"
                    } else {
                        "no rustfmt bin, try running `cargo build` before testing"
                    }
                );
                me
            };
        }
        &RUSTFMT_PATH
    }

    #[test]
    fn verify_check_works() {
        init_log();
        let temp_file = make_temp_file("temp_check.rs");

        Command::new(rustfmt())
            .arg("--check")
            .arg(&temp_file.path)
            .status()
            .expect("run with check option failed");
    }

    #[test]
    fn verify_check_works_with_stdin() {
        init_log();

        let mut child = Command::new(rustfmt())
            .arg("--check")
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run with check option failed");

        {
            let stdin = child.stdin.as_mut().expect("Failed to open stdin");
            stdin
                .write_all(b"fn main() {}\n")
                .expect("Failed to write to rustfmt --check");
        }
        let output = child
            .wait_with_output()
            .expect("Failed to wait on rustfmt child");
        assert!(output.status.success());
    }

    #[test]
    fn verify_check_l_works_with_stdin() {
        init_log();

        let mut child = Command::new(rustfmt())
            .arg("--check")
            .arg("-l")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run with check option failed");

        {
            let stdin = child.stdin.as_mut().expect("Failed to open stdin");
            stdin
                .write_all(b"fn main()\n{}\n")
                .expect("Failed to write to rustfmt --check");
        }
        let output = child
            .wait_with_output()
            .expect("Failed to wait on rustfmt child");
        assert!(!output.status.success());
        assert_eq!(std::str::from_utf8(&output.stdout).unwrap(), "<stdin>\n");
    }

    #[cfg(nightly)]
    #[test]
    fn verify_error_on_unformatted() {
        init_log();

        let mut child = Command::new(rustfmt())
            .arg("--error-on-unformatted")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run with check option failed");

        {
            let stdin = child.stdin.as_mut().expect("Failed to open stdin");
            stdin
                .write_all(b"fn main()\n{}\n")
                .expect("Failed to write to rustfmt --check");
        }

        let output = child
            .wait_with_output()
            .expect("Failed to wait on rustfmt child");
        assert!(output.status.success());
    }

    #[cfg(test)]
    mod force {
        use super::*;

        const CONTENTS: &[u8] = br#"
        #![rustfmt::max_width(120)]
        fn foo() {
        println!("bar");
        }
        "#;

        const FORMATTED_CONTENTS: &[u8] = br#"
#![rustfmt::max_width(120)]
fn foo() {
    println!("bar");
}
"#;

        #[test]
        fn verify_default() {
            init_log();
            let temp_file = make_temp_file_with_contents("temp_invalid_attrs.rs", CONTENTS);

            let child = Command::new(rustfmt())
                .arg(&temp_file.path)
                .stderr(Stdio::piped())
                .spawn()
                .expect("run without --force option failed");
            let output = child
                .wait_with_output()
                .expect("Failed to wait on rustfmt child");

            assert!(!output.status.success());
            assert_temp_file_contents(CONTENTS, temp_file);
        }

        #[test]
        fn verify_enabled() {
            init_log();
            let temp_file = make_temp_file_with_contents("temp_invalid_attrs_enabled.rs", CONTENTS);

            let child = Command::new(rustfmt())
                .arg(&temp_file.path)
                .arg("--force")
                .stderr(Stdio::piped())
                .spawn()
                .expect("run with --force option failed");
            let output = child
                .wait_with_output()
                .expect("Failed to wait on rustfmt child");
            assert!(output.status.success());
            assert_temp_file_contents(FORMATTED_CONTENTS, temp_file);
        }

        #[test]
        fn verify_default_stdin() {
            if !rustfmt_nightly::is_nightly_channel!() {
                return;
            }
            init_log();

            let mut child = Command::new(rustfmt())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("run with check option failed");

            {
                let stdin = child.stdin.as_mut().expect("Failed to open stdin");
                stdin
                    .write_all(CONTENTS)
                    .expect("Failed to write to rustfmt --check");
            }
            let output = child
                .wait_with_output()
                .expect("Failed to wait on rustfmt child");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let exp_err = vec![
                "\u{1b}[1;38;5;9merror\u{1b}[0m: \u{1b}[1minvalid attribute",
                "\u{1b}[0m\n \u{1b}[1;38;5;12m-->\u{1b}[0m <stdin>:2\n",
                "\u{1b}[1;38;5;12m  |\u{1b}[0m\n",
                "\u{1b}[1;38;5;12m2 |\u{1b}[0m         #![rustfmt::max_width(120)]\n",
                "\u{1b}[1;38;5;12m  |\u{1b}[0m\n\n",
                "\u{1b}[1;38;5;11mwarning\u{1b}[0m: \u{1b}[1mrustfmt has failed to format. ",
                "See previous 1 errors.",
                "\u{1b}[0m\n\n",
            ]
            .join("");
            assert_eq!(String::from_utf8(output.stderr).unwrap(), exp_err);
        }

        #[test]
        fn verify_enabled_stdin() {
            init_log();

            let mut child = Command::new(rustfmt())
                .arg("--force")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("run with check option failed");

            {
                let stdin = child.stdin.as_mut().expect("Failed to open stdin");
                stdin
                    .write_all(CONTENTS)
                    .expect("Failed to write to rustfmt --check");
            }
            let output = child
                .wait_with_output()
                .expect("Failed to wait on rustfmt child");
            assert!(output.status.success());
            assert_eq!(&output.stdout, &FORMATTED_CONTENTS);
        }
    }
}
