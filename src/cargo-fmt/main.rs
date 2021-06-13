// Inspired by Paul Woolcock's cargo-fmt (https://github.com/pwoolcoc/cargo-fmt/).

#![deny(warnings)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    bin_name = "cargo fmt",
    about = "This utility formats all bin and lib files of \
             the current crate using rustfmt."
)]
pub struct Opts {
    /// No output printed to stdout
    #[structopt(short = "q", long = "quiet")]
    quiet: bool,

    /// Use verbose output
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,

    /// Print rustfmt version and exit
    #[structopt(long = "version")]
    version: bool,

    /// Specify package to format
    #[structopt(short = "p", long = "package", value_name = "package")]
    packages: Vec<String>,

    /// Specify path to Cargo.toml
    #[structopt(long = "manifest-path", value_name = "manifest-path")]
    manifest_path: Option<String>,

    /// Specify message-format: short|json|human
    #[structopt(long = "message-format", value_name = "message-format")]
    message_format: Option<String>,

    /// Options passed to rustfmt
    ///
    /// To see all rustfmt options, run `rustfmt --help'.
    /// To see all rustfmt configuration options, please visit https://rust-lang.github.io/rustfmt.
    // 'raw = true' to make `--` explicit.
    #[structopt(name = "rustfmt_options", raw(true))]
    rustfmt_options: Vec<String>,

    /// Format all packages, and also their local path-based dependencies
    #[structopt(long = "all")]
    format_all: bool,

    /// Run rustfmt in check mode
    #[structopt(long = "check")]
    check: bool,

    /// Format files located within subdirectories of `tests` directory
    #[structopt(long = "include-nested-test-files")]
    include_nested_test_files: bool,
}

fn main() {
    let exit_status = execute();
    std::io::stdout().flush().unwrap();
    std::process::exit(exit_status);
}

const SUCCESS: i32 = 0;
const FAILURE: i32 = 1;

fn execute() -> i32 {
    // Drop extra `fmt` argument provided by `cargo`.
    let mut found_fmt = false;
    let args = env::args().filter(|x| {
        if found_fmt {
            true
        } else {
            found_fmt = x == "fmt";
            x != "fmt"
        }
    });

    let opts = Opts::from_iter(args);

    let verbosity = match (opts.verbose, opts.quiet) {
        (false, false) => Verbosity::Normal,
        (false, true) => Verbosity::Quiet,
        (true, false) => Verbosity::Verbose,
        (true, true) => {
            print_usage_to_stderr("quiet mode and verbose mode are not compatible");
            return FAILURE;
        }
    };

    if opts.version {
        return handle_command_status(get_rustfmt_info(&[String::from("--version")]));
    }
    if opts
        .rustfmt_options
        .iter()
        .any(|s| is_status_options(s.as_str()))
    {
        return handle_command_status(get_rustfmt_info(&opts.rustfmt_options));
    }

    let strategy = CargoFmtStrategy::from_opts(&opts);
    let mut rustfmt_args = opts.rustfmt_options.to_owned();
    if let Err(ref msg) = build_rustfmt_args(&opts, &mut rustfmt_args) {
        print_usage_to_stderr(msg);
        return FAILURE;
    }
    let include_nested_test_files = opts.include_nested_test_files;

    if let Some(specified_manifest_path) = opts.manifest_path {
        if !specified_manifest_path.ends_with("Cargo.toml") {
            print_usage_to_stderr("the manifest-path must be a path to a Cargo.toml file");
            return FAILURE;
        }
        let manifest_path = PathBuf::from(specified_manifest_path);
        handle_command_status(format_crate(
            verbosity,
            &strategy,
            rustfmt_args,
            Some(&manifest_path),
            include_nested_test_files,
        ))
    } else {
        handle_command_status(format_crate(
            verbosity,
            &strategy,
            rustfmt_args,
            None,
            include_nested_test_files,
        ))
    }
}

fn is_status_options(s: &str) -> bool {
    ["--print-config", "-h", "--help", "-V", "--version"].contains(&s)
        || s.starts_with("--help=")
        || s.starts_with("--print-config=")
}

fn rustfmt_command() -> Command {
    let rustfmt_var = env::var_os("RUSTFMT");
    let rustfmt = match &rustfmt_var {
        Some(rustfmt) => rustfmt,
        None => OsStr::new("rustfmt"),
    };
    Command::new(rustfmt)
}

fn build_rustfmt_args(opts: &Opts, rustfmt_args: &mut Vec<String>) -> Result<(), String> {
    let mut contains_check = false;
    let mut contains_emit_mode = false;
    let mut contains_list_files = false;
    let mut contains_recursive = false;

    for arg in rustfmt_args.iter() {
        if arg.starts_with("--emit") {
            contains_emit_mode = true;
        }
        if arg == "--check" {
            contains_check = true;
        }
        if arg == "-l" || arg == "--files-with-diff" {
            contains_list_files = true;
        }
        if arg == "-r" || arg == "--recursive" {
            contains_recursive = true;
        }
    }

    if opts.check && !contains_check {
        rustfmt_args.push(String::from("--check"));
    }

    if !contains_recursive {
        rustfmt_args.push(String::from("--recursive"));
    }

    if let Some(ref format) = opts.message_format {
        return match format.as_ref() {
            "short" => {
                if !contains_list_files {
                    rustfmt_args.push(String::from("-l"));
                }
                Ok(())
            }
            "json" => {
                if contains_emit_mode {
                    return Err(String::from(
                        "cannot include --emit arg when --message-format is set to json",
                    ));
                }
                if contains_check {
                    return Err(String::from(
                        "cannot include --check arg when --message-format is set to json",
                    ));
                }
                rustfmt_args.push(String::from("--emit"));
                rustfmt_args.push(String::from("json"));
                Ok(())
            }
            "human" => Ok(()),
            _ => {
                return Err(format!(
                    "invalid --message-format value: {}. Allowed values are: short|json|human",
                    format
                ));
            }
        };
    }

    Ok(())
}

fn print_usage_to_stderr(reason: &str) {
    eprintln!("{}", reason);
    let app = Opts::clap();
    app.after_help("")
        .write_help(&mut io::stderr())
        .expect("failed to write to stderr");
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verbosity {
    Verbose,
    Normal,
    Quiet,
}

fn handle_command_status(status: Result<i32, io::Error>) -> i32 {
    match status {
        Err(e) => {
            print_usage_to_stderr(&e.to_string());
            FAILURE
        }
        Ok(status) => status,
    }
}

fn get_rustfmt_info(args: &[String]) -> Result<i32, io::Error> {
    let mut command = rustfmt_command()
        .stdout(std::process::Stdio::inherit())
        .args(args)
        .spawn()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(
                io::ErrorKind::Other,
                "Could not run rustfmt, please make sure it is in your PATH.",
            ),
            _ => e,
        })?;
    let result = command.wait()?;
    if result.success() {
        Ok(SUCCESS)
    } else {
        Ok(result.code().unwrap_or(SUCCESS))
    }
}

fn format_crate(
    verbosity: Verbosity,
    strategy: &CargoFmtStrategy,
    rustfmt_args: Vec<String>,
    manifest_path: Option<&Path>,
    include_nested_test_files: bool,
) -> Result<i32, io::Error> {
    let targets = get_targets(strategy, manifest_path, include_nested_test_files)?;

    // Currently only bin and lib files get formatted.
    run_rustfmt(&targets, &rustfmt_args, verbosity)
}

/// Target uses a `path` field for equality and hashing.
#[derive(Debug)]
pub struct Target {
    /// A path to the main source file of the target.
    path: PathBuf,
    /// A kind of target (e.g., lib, bin, example, ...).
    kind: String,
    /// Rust edition for this target.
    edition: String,
    /// Rust files residing within subdirectories of the tests directory.
    nested_int_test_files: Vec<PathBuf>,
}

impl Target {
    pub fn from_target(
        target: &cargo_metadata::Target,
        nested_int_test_files: Option<Vec<PathBuf>>,
    ) -> Self {
        let path = PathBuf::from(&target.src_path);
        let canonicalized = dunce::canonicalize(&path).unwrap_or(path);
        let test_files = nested_int_test_files.unwrap_or_else(Vec::new);

        Self {
            path: canonicalized,
            kind: target.kind[0].clone(),
            edition: target.edition.clone(),
            nested_int_test_files: test_files,
        }
    }
}

impl PartialEq for Target {
    fn eq(&self, other: &Target) -> bool {
        self.path == other.path
    }
}

impl PartialOrd for Target {
    fn partial_cmp(&self, other: &Target) -> Option<Ordering> {
        Some(self.path.cmp(&other.path))
    }
}

impl Ord for Target {
    fn cmp(&self, other: &Target) -> Ordering {
        self.path.cmp(&other.path)
    }
}

impl Eq for Target {}

impl Hash for Target {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CargoFmtStrategy {
    /// Format every packages and dependencies.
    All,
    /// Format packages that are specified by the command line argument.
    Some(Vec<String>),
    /// Format the root packages only.
    Root,
}

impl CargoFmtStrategy {
    pub fn from_opts(opts: &Opts) -> CargoFmtStrategy {
        match (opts.format_all, opts.packages.is_empty()) {
            (false, true) => CargoFmtStrategy::Root,
            (true, _) => CargoFmtStrategy::All,
            (false, false) => CargoFmtStrategy::Some(opts.packages.clone()),
        }
    }
}

/// Based on the specified `CargoFmtStrategy`, returns a set of main source files.
fn get_targets(
    strategy: &CargoFmtStrategy,
    manifest_path: Option<&Path>,
    include_nested_test_files: bool,
) -> Result<BTreeSet<Target>, io::Error> {
    let mut targets = BTreeSet::new();

    match *strategy {
        CargoFmtStrategy::Root => {
            get_targets_root_only(manifest_path, &mut targets, include_nested_test_files)?
        }
        CargoFmtStrategy::All => get_targets_recursive(
            manifest_path,
            &mut targets,
            &mut BTreeSet::new(),
            include_nested_test_files,
        )?,
        CargoFmtStrategy::Some(ref hitlist) => get_targets_with_hitlist(
            manifest_path,
            hitlist,
            &mut targets,
            include_nested_test_files,
        )?,
    }

    if targets.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to find targets".to_owned(),
        ))
    } else {
        Ok(targets)
    }
}

fn get_targets_root_only(
    manifest_path: Option<&Path>,
    mut targets: &mut BTreeSet<Target>,
    include_nested_test_files: bool,
) -> Result<(), io::Error> {
    let metadata = get_cargo_metadata(manifest_path, false)?;
    let workspace_root_path = dunce::canonicalize(PathBuf::from(&metadata.workspace_root))?;
    let (in_workspace_root, current_dir_manifest) = if let Some(target_manifest) = manifest_path {
        (
            workspace_root_path.as_path() == target_manifest,
            dunce::canonicalize(target_manifest)?,
        )
    } else {
        let current_dir = dunce::canonicalize(env::current_dir()?)?;
        (
            workspace_root_path == current_dir,
            current_dir.join("Cargo.toml"),
        )
    };

    let (package_targets, package_manifest_path) = match metadata.packages.len() {
        1 => {
            let package = metadata.packages.into_iter().next().unwrap();
            (package.targets, PathBuf::from(&package.manifest_path))
        }
        _ => (
            metadata
                .packages
                .into_iter()
                .filter(|p| {
                    in_workspace_root
                        || dunce::canonicalize(PathBuf::from(&p.manifest_path)).unwrap_or_default()
                            == current_dir_manifest
                })
                .map(|p| p.targets)
                .flatten()
                .collect(),
            current_dir_manifest,
        ),
    };

    add_targets(
        &package_manifest_path,
        &package_targets,
        &mut targets,
        include_nested_test_files,
    )?;

    Ok(())
}

fn get_targets_recursive(
    manifest_path: Option<&Path>,
    mut targets: &mut BTreeSet<Target>,
    visited: &mut BTreeSet<String>,
    include_nested_test_files: bool,
) -> Result<(), io::Error> {
    let metadata = get_cargo_metadata(manifest_path, false)?;
    let metadata_with_deps = get_cargo_metadata(manifest_path, true)?;

    for package in metadata.packages {
        add_targets(
            &package.manifest_path,
            &package.targets,
            &mut targets,
            include_nested_test_files,
        )?;

        // Look for local dependencies.
        for dependency in package.dependencies {
            if dependency.source.is_some() || visited.contains(&dependency.name) {
                continue;
            }

            let dependency_package = metadata_with_deps
                .packages
                .iter()
                .find(|p| p.name == dependency.name && p.source.is_none());

            if let Some(dependency_package) = &dependency_package {
                // Dependency is a member of the current workspace, no need to
                // recursively call `cargo metadata` on it.
                if metadata.workspace_members.contains(&dependency_package.id) {
                    continue;
                }
            }

            let manifest_path = match dependency_package {
                Some(p) => PathBuf::from(&p.manifest_path),
                None => {
                    let mut package_manifest_path = PathBuf::from(&package.manifest_path);
                    package_manifest_path.pop();
                    package_manifest_path.push(&dependency.name);
                    package_manifest_path.push("Cargo.toml");
                    package_manifest_path
                }
            };

            if manifest_path.exists() {
                visited.insert(dependency.name);
                get_targets_recursive(
                    Some(&manifest_path),
                    &mut targets,
                    visited,
                    include_nested_test_files,
                )?;
            }
        }
    }

    Ok(())
}

fn get_targets_with_hitlist(
    manifest_path: Option<&Path>,
    hitlist: &[String],
    mut targets: &mut BTreeSet<Target>,
    include_nested_test_files: bool,
) -> Result<(), io::Error> {
    let metadata = get_cargo_metadata(manifest_path, false)?;

    let mut workspace_hitlist: BTreeSet<&String> = BTreeSet::from_iter(hitlist);

    for package in metadata.packages {
        if workspace_hitlist.remove(&package.name) {
            add_targets(
                &package.manifest_path,
                &package.targets,
                &mut targets,
                include_nested_test_files,
            )?;
        }
    }

    if workspace_hitlist.is_empty() {
        Ok(())
    } else {
        let package = workspace_hitlist.iter().next().unwrap();
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("package `{}` is not a member of the workspace", package),
        ))
    }
}

fn add_targets(
    manifest_path: &PathBuf,
    target_paths: &[cargo_metadata::Target],
    targets: &mut BTreeSet<Target>,
    include_nested_test_files: bool,
) -> Result<(), io::Error> {
    let mut test_files_added = false;
    for target in target_paths {
        // Packages often have more than one `test` target,
        // so only add the nested files for the first one.
        let check_for_nested_test_files = include_nested_test_files
            && !test_files_added
            && target.kind.iter().any(|t| t == "test");

        if !check_for_nested_test_files {
            targets.insert(Target::from_target(&target, None));
            continue;
        }

        if let Some(package_dir) = manifest_path.parent() {
            let target_dir = package_dir.join("tests");
            test_files_added = true;
            let test_files = get_nested_integration_test_files(&target_dir, &target_dir);
            if test_files.is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Error encountered while searching for nested integration test files",
                ));
            }
            targets.insert(Target::from_target(&target, test_files));
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unable to determine root `tests` directory for /
                nested integration test file discovery",
            ));
        }
    }

    Ok(())
}

// Returns a `Vec` containing `PathBuf`s of nested .rs files within subdirectories
// of the `tests` directory for a given package.
// https://github.com/rust-lang/rustfmt/issues/1820
fn get_nested_integration_test_files(path: &Path, root_dir: &Path) -> Option<Vec<PathBuf>> {
    if !path.is_dir() {
        return Some(vec![]);
    }
    let mut files = vec![];
    let dir = fs::read_dir(path).ok()?;

    for dir_entry in dir {
        let entry_path = dir_entry.ok()?.path();
        if entry_path.is_dir() {
            files.append(&mut get_nested_integration_test_files(
                &entry_path,
                &root_dir,
            )?);
        } else if entry_path.extension().map_or(false, |f| f == "rs") && path != root_dir {
            files.push(entry_path);
        }
    }
    Some(files)
}

fn run_rustfmt(
    targets: &BTreeSet<Target>,
    fmt_args: &[String],
    verbosity: Verbosity,
) -> Result<i32, io::Error> {
    let by_edition = targets
        .iter()
        .inspect(|t| {
            if verbosity == Verbosity::Verbose {
                println!("[{} ({})] {:?}", t.kind, t.edition, t.path)
            }
        })
        .fold(BTreeMap::new(), |mut h, t| {
            let entry = h.entry(&t.edition).or_insert_with(Vec::new);
            for test_file in &t.nested_int_test_files {
                entry.push(test_file)
            }
            entry.push(&t.path);
            h
        });

    let mut status = vec![];
    for (edition, files) in by_edition {
        let stdout = if verbosity == Verbosity::Quiet {
            std::process::Stdio::null()
        } else {
            std::process::Stdio::inherit()
        };

        if verbosity == Verbosity::Verbose {
            print!("rustfmt");
            print!(" --edition {}", edition);
            fmt_args.iter().for_each(|f| print!(" {}", f));
            files.iter().for_each(|f| print!(" {}", f.display()));
            println!();
        }

        let mut command = rustfmt_command()
            .stdout(stdout)
            .args(files)
            .args(&["--edition", edition])
            .args(fmt_args)
            .spawn()
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => io::Error::new(
                    io::ErrorKind::Other,
                    "Could not run rustfmt, please make sure it is in your PATH.",
                ),
                _ => e,
            })?;

        status.push(command.wait()?);
    }

    Ok(status
        .iter()
        .filter_map(|s| if s.success() { None } else { s.code() })
        .next()
        .unwrap_or(SUCCESS))
}

fn get_cargo_metadata(
    manifest_path: Option<&Path>,
    include_deps: bool,
) -> Result<cargo_metadata::Metadata, io::Error> {
    let mut cmd = cargo_metadata::MetadataCommand::new();
    if !include_deps {
        cmd.no_deps();
    }
    if let Some(manifest_path) = manifest_path {
        cmd.manifest_path(manifest_path);
    }
    cmd.other_options(vec![String::from("--offline")]);

    match cmd.exec() {
        Ok(metadata) => Ok(metadata),
        Err(_) => {
            cmd.other_options(vec![]);
            match cmd.exec() {
                Ok(metadata) => Ok(metadata),
                Err(error) => Err(io::Error::new(io::ErrorKind::Other, error.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod cargo_fmt_tests {
    use super::*;

    #[test]
    fn default_options() {
        let empty: Vec<String> = vec![];
        let o = Opts::from_iter(&empty);
        assert_eq!(false, o.quiet);
        assert_eq!(false, o.verbose);
        assert_eq!(false, o.version);
        assert_eq!(false, o.check);
        assert_eq!(empty, o.packages);
        assert_eq!(empty, o.rustfmt_options);
        assert_eq!(false, o.format_all);
        assert_eq!(None, o.manifest_path);
        assert_eq!(None, o.message_format);
    }

    #[test]
    fn good_options() {
        let o = Opts::from_iter(&[
            "test",
            "-q",
            "-p",
            "p1",
            "-p",
            "p2",
            "--message-format",
            "short",
            "--check",
            "--",
            "--edition",
            "2018",
        ]);
        assert_eq!(true, o.quiet);
        assert_eq!(false, o.verbose);
        assert_eq!(false, o.version);
        assert_eq!(true, o.check);
        assert_eq!(vec!["p1", "p2"], o.packages);
        assert_eq!(vec!["--edition", "2018"], o.rustfmt_options);
        assert_eq!(false, o.format_all);
        assert_eq!(Some(String::from("short")), o.message_format);
    }

    #[test]
    fn unexpected_option() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "unexpected"])
                .is_err()
        );
    }

    #[test]
    fn unexpected_flag() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "--flag"])
                .is_err()
        );
    }

    #[test]
    fn mandatory_separator() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "--emit"])
                .is_err()
        );
        assert!(
            !Opts::clap()
                .get_matches_from_safe(&["test", "--", "--emit"])
                .is_err()
        );
    }

    #[test]
    fn multiple_packages_one_by_one() {
        let o = Opts::from_iter(&[
            "test",
            "-p",
            "package1",
            "--package",
            "package2",
            "-p",
            "package3",
        ]);
        assert_eq!(3, o.packages.len());
    }

    #[test]
    fn multiple_packages_grouped() {
        let o = Opts::from_iter(&[
            "test",
            "--package",
            "package1",
            "package2",
            "-p",
            "package3",
            "package4",
        ]);
        assert_eq!(4, o.packages.len());
    }

    #[test]
    fn empty_packages_1() {
        assert!(Opts::clap().get_matches_from_safe(&["test", "-p"]).is_err());
    }

    #[test]
    fn empty_packages_2() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "-p", "--", "--check"])
                .is_err()
        );
    }

    #[test]
    fn empty_packages_3() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "-p", "--verbose"])
                .is_err()
        );
    }

    #[test]
    fn empty_packages_4() {
        assert!(
            Opts::clap()
                .get_matches_from_safe(&["test", "-p", "--check"])
                .is_err()
        );
    }

    mod build_rustfmt_args_tests {
        use super::*;

        #[test]
        fn invalid_message_format() {
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "awesome"]);
            assert_eq!(
                build_rustfmt_args(&cargo_fmt_opts, &mut vec![]),
                Err(String::from(
                    "invalid --message-format value: awesome. Allowed values are: short|json|human"
                )),
            );
        }

        #[test]
        fn json_message_format_and_check_arg() {
            let mut rustfmt_args = vec![String::from("--check")];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "json"]);
            assert_eq!(
                build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args),
                Err(String::from(
                    "cannot include --check arg when --message-format is set to json"
                )),
            );
        }

        #[test]
        fn json_message_format_and_emit_arg() {
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "json"]);
            let mut rustfmt_args = vec![String::from("--emit"), String::from("checkstyle")];
            assert_eq!(
                build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args),
                Err(String::from(
                    "cannot include --emit arg when --message-format is set to json"
                )),
            );
        }

        #[test]
        fn json_message_format() {
            let mut rustfmt_args = vec![
                String::from("--edition"),
                String::from("2018"),
                String::from("--recursive"),
            ];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "json"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![
                    String::from("--edition"),
                    String::from("2018"),
                    String::from("--recursive"),
                    String::from("--emit"),
                    String::from("json"),
                ]
            );
        }

        #[test]
        fn human_message_format() {
            let exp_args = vec![
                String::from("--emit"),
                String::from("json"),
                String::from("--recursive"),
            ];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "human"]);
            let mut rustfmt_args = exp_args.clone();
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(rustfmt_args, exp_args);
        }

        #[test]
        fn short_message_format() {
            let mut rustfmt_args = vec![String::from("--check"), String::from("--recursive")];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "short"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![
                    String::from("--check"),
                    String::from("--recursive"),
                    String::from("-l"),
                ],
            );
        }

        #[test]
        fn short_message_format_included_short_list_files_flag() {
            let mut rustfmt_args = vec![
                String::from("--check"),
                String::from("-l"),
                String::from("--recursive"),
            ];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "short"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![
                    String::from("--check"),
                    String::from("-l"),
                    String::from("--recursive"),
                ],
            );
        }

        #[test]
        fn short_message_format_included_long_list_files_flag() {
            let mut rustfmt_args = vec![
                String::from("--check"),
                String::from("--files-with-diff"),
                String::from("--recursive"),
            ];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--message-format", "short"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![
                    String::from("--check"),
                    String::from("--files-with-diff"),
                    String::from("--recursive"),
                ]
            );
        }

        #[test]
        fn recursive_shorthand_not_duplicated() {
            let mut rustfmt_args = vec![String::from("-r")];
            let empty: Vec<String> = vec![];
            assert!(build_rustfmt_args(&Opts::from_iter(&empty), &mut rustfmt_args).is_ok());
            assert_eq!(rustfmt_args, vec![String::from("-r")]);
        }

        #[test]
        fn recursive_long_not_duplicated() {
            let mut rustfmt_args = vec![String::from("--recursive")];
            let empty: Vec<String> = vec![];
            assert!(build_rustfmt_args(&Opts::from_iter(&empty), &mut rustfmt_args).is_ok());
            assert_eq!(rustfmt_args, vec![String::from("--recursive")]);
        }

        #[test]
        fn recursive_added() {
            let mut rustfmt_args = vec![];
            let empty: Vec<String> = vec![];
            assert!(build_rustfmt_args(&Opts::from_iter(&empty), &mut rustfmt_args).is_ok());
            assert_eq!(rustfmt_args, vec![String::from("--recursive")]);
        }

        #[test]
        fn check_not_duplicated_when_included_in_cargo_fmt() {
            let mut rustfmt_args = vec![String::from("--check"), String::from("--recursive")];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--check"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![String::from("--check"), String::from("--recursive")],
            );
        }

        #[test]
        fn check_still_passed_through_when_not_included_in_cargo_fmt() {
            let mut rustfmt_args = vec![String::from("--check"), String::from("--recursive")];
            let empty: Vec<String> = vec![];
            assert!(build_rustfmt_args(&Opts::from_iter(&empty), &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![String::from("--check"), String::from("--recursive")],
            );
        }

        #[test]
        fn check_added() {
            let mut rustfmt_args = vec![String::from("--recursive")];
            let cargo_fmt_opts = Opts::from_iter(&["test", "--check"]);
            assert!(build_rustfmt_args(&cargo_fmt_opts, &mut rustfmt_args).is_ok());
            assert_eq!(
                rustfmt_args,
                vec![String::from("--recursive"), String::from("--check")],
            );
        }

        #[test]
        fn check_not_added_when_flag_disabled() {
            let mut rustfmt_args = vec![String::from("--recursive")];
            let empty: Vec<String> = vec![];
            assert!(build_rustfmt_args(&Opts::from_iter(&empty), &mut rustfmt_args).is_ok());
            assert_eq!(rustfmt_args, vec![String::from("--recursive")]);
        }
    }

    mod get_nested_integration_test_files_tests {
        use super::*;

        #[test]
        fn returns_no_files_if_root_not_dir() {
            let target_dir = PathBuf::from("tests/nested-test-files/no-test-dir/Cargo.toml");
            assert_eq!(
                Some(Vec::new() as Vec<PathBuf>),
                get_nested_integration_test_files(&target_dir, &target_dir),
            )
        }

        #[test]
        fn returns_no_files_if_tests_has_no_nested_files() {
            let target_dir = Path::new("tests/nested-test-files/only-root-level-tests/tests");
            assert_eq!(
                Some(Vec::new() as Vec<PathBuf>),
                get_nested_integration_test_files(&target_dir, &target_dir),
            )
        }

        #[test]
        fn returns_nested_files() {
            let target_dir = Path::new("tests/nested-test-files/root-and-nested-tests/tests");
            let exp_baz = PathBuf::from(
                "tests/nested-test-files/root-and-nested-tests/tests/nested/deeply-nested/baz.rs",
            );
            let exp_foo_bar = PathBuf::from(
                "tests/nested-test-files/root-and-nested-tests/tests/nested/foo_bar.rs",
            );
            let exp_other = PathBuf::from(
                "tests/nested-test-files/root-and-nested-tests/tests/nested/other.rs",
            );
            let files = get_nested_integration_test_files(&target_dir, &target_dir).unwrap();
            assert_eq!(3, files.len());
            assert!(files.contains(&exp_baz));
            assert!(files.contains(&exp_foo_bar));
            assert!(files.contains(&exp_other));
        }
    }

    mod get_targets_tests {
        use super::*;

        fn create_stub_target(
            src_path: &str,
            test_files: Vec<PathBuf>,
            kind: &str,
            edition: &str,
        ) -> Target {
            let path = PathBuf::from(src_path);
            let canonicalized = dunce::canonicalize(&path).unwrap_or(path);
            Target {
                path: canonicalized,
                kind: String::from(kind),
                edition: String::from(edition),
                nested_int_test_files: test_files,
            }
        }

        mod nested_test_files {
            use super::*;

            #[test]
            fn does_not_include_nested_test_files_when_disabled() {
                let edition = "2018";
                let project_dir_base = "tests/nested-test-files/root-and-nested-tests";
                let manifest_path = PathBuf::from(format!("{}/Cargo.toml", project_dir_base));
                let mut exp_targets: BTreeSet<Target> = BTreeSet::new();
                exp_targets.insert(create_stub_target(
                    &format!("{}/src/lib.rs", project_dir_base),
                    vec![],
                    "lib",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/bar.rs", project_dir_base),
                    vec![],
                    "test",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/foo.rs", project_dir_base),
                    vec![],
                    "test",
                    edition,
                ));
                let strategy = CargoFmtStrategy::Root;
                let act_targets = get_targets(&strategy, Some(&manifest_path), false);
                assert_eq!(act_targets.unwrap(), exp_targets);
            }

            #[test]
            fn does_include_nested_test_files_when_enabled() {
                let edition = "2018";
                let project_dir_base = "tests/nested-test-files/root-and-nested-tests";
                let manifest_path = PathBuf::from(format!("{}/Cargo.toml", project_dir_base));
                let exp_baz = PathBuf::from(format!(
                    "{}/tests/nested/deeply-nested/baz.rs",
                    project_dir_base
                ));
                let exp_foo_bar =
                    PathBuf::from(format!("{}/tests/nested/foo_bar.rs", project_dir_base));
                let exp_other =
                    PathBuf::from(format!("{}/tests/nested/other.rs", project_dir_base));
                let mut exp_targets: BTreeSet<Target> = BTreeSet::new();
                exp_targets.insert(create_stub_target(
                    &format!("{}/src/lib.rs", project_dir_base),
                    vec![],
                    "lib",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/bar.rs", project_dir_base),
                    vec![exp_baz, exp_foo_bar, exp_other],
                    "test",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/foo.rs", project_dir_base),
                    vec![],
                    "test",
                    edition,
                ));
                let strategy = CargoFmtStrategy::Root;
                let act_targets = get_targets(&strategy, Some(&manifest_path), true);
                assert_eq!(act_targets.unwrap(), exp_targets);
            }

            #[test]
            fn returns_correct_targets_with_empty_tests_dir() {
                let edition = "2015";
                let project_dir_base = "tests/nested-test-files/empty-tests-dir";
                let manifest_path = PathBuf::from(format!("{}/Cargo.toml", project_dir_base));
                let mut exp_targets: BTreeSet<Target> = BTreeSet::new();
                exp_targets.insert(create_stub_target(
                    &format!("{}/src/lib.rs", project_dir_base),
                    vec![],
                    "lib",
                    edition,
                ));
                let strategy = CargoFmtStrategy::Root;
                let act_targets = get_targets(&strategy, Some(&manifest_path), true);
                assert_eq!(act_targets.unwrap(), exp_targets);
            }

            #[test]
            fn returns_correct_targets_with_no_tests_dir() {
                let edition = "2015";
                let project_dir_base = "tests/nested-test-files/no-tests-dir";
                let manifest_path = PathBuf::from(format!("{}/Cargo.toml", project_dir_base));
                let mut exp_targets: BTreeSet<Target> = BTreeSet::new();
                exp_targets.insert(create_stub_target(
                    &format!("{}/src/lib.rs", project_dir_base),
                    vec![],
                    "lib",
                    edition,
                ));
                let strategy = CargoFmtStrategy::Root;
                let act_targets = get_targets(&strategy, Some(&manifest_path), true);
                assert_eq!(act_targets.unwrap(), exp_targets);
            }

            #[test]
            fn returns_correct_targets_with_only_root_level_tests() {
                let edition = "2015";
                let project_dir_base = "tests/nested-test-files/only-root-level-tests-dir";
                let manifest_path = PathBuf::from(format!("{}/Cargo.toml", project_dir_base));
                let mut exp_targets: BTreeSet<Target> = BTreeSet::new();
                exp_targets.insert(create_stub_target(
                    &format!("{}/src/lib.rs", project_dir_base),
                    vec![],
                    "lib",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/bar.rs", project_dir_base),
                    vec![],
                    "test",
                    edition,
                ));
                exp_targets.insert(create_stub_target(
                    &format!("{}/tests/foo.rs", project_dir_base),
                    vec![],
                    "test",
                    edition,
                ));
                let strategy = CargoFmtStrategy::Root;
                let act_targets = get_targets(&strategy, Some(&manifest_path), true);
                assert_eq!(act_targets.unwrap(), exp_targets);
            }
        }
    }
}
