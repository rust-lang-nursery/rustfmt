use super::*;
use crate::emitter::{Color, EmitterConfig, Verbosity};
use rustfmt_diff::{make_diff, print_diff};

pub struct DiffEmitter {
    color: Color,
    verbosity: Verbosity,
    print_filename: bool,
}

impl DiffEmitter {
    pub fn new(
        EmitterConfig {
            color,
            verbosity,
            print_filename,
            ..
        }: EmitterConfig,
    ) -> Self {
        Self {
            color,
            verbosity,
            print_filename,
        }
    }
}

impl Emitter for DiffEmitter {
    fn emit_formatted_file(
        &mut self,
        output: &mut dyn Write,
        FormattedFile {
            filename,
            original_text,
            formatted_text,
        }: FormattedFile<'_>,
        _newline_style: NewlineStyle,
    ) -> Result<EmitterResult, EmitterError> {
        const CONTEXT_SIZE: usize = 3;
        let mismatch = make_diff(&original_text, formatted_text, CONTEXT_SIZE);
        let has_diff = !mismatch.is_empty();

        if has_diff {
            if self.print_filename {
                writeln!(output, "{}", filename)?;
            } else {
                print_diff(
                    mismatch,
                    |line_num| format!("Diff in {}:{}:", filename, line_num),
                    self.color,
                    self.verbosity,
                );
            }
        } else if original_text != formatted_text {
            // This occurs when the only difference between the original and formatted values
            // is the newline style. This happens because The make_diff function compares the
            // original and formatted values line by line, independent of line endings.
            writeln!(output, "Incorrect newline style in {}", filename)?;
            return Ok(EmitterResult { has_diff: true });
        }

        Ok(EmitterResult { has_diff })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileName;
    use std::path::PathBuf;

    #[test]
    fn does_not_print_when_no_files_reformatted() {
        let mut writer = Vec::new();
        let mut emitter = DiffEmitter::new(EmitterConfig::default());
        let result = emitter
            .emit_formatted_file(
                &mut writer,
                FormattedFile {
                    filename: &FileName::Real(PathBuf::from("src/lib.rs")),
                    original_text: "fn empty() {}\n",
                    formatted_text: "fn empty() {}\n",
                },
                NewlineStyle::default(),
            )
            .unwrap();
        assert_eq!(result.has_diff, false);
        assert_eq!(writer.len(), 0);
    }

    #[test]
    fn prints_file_names_when_config_is_enabled() {
        let bin_file = "src/bin.rs";
        let bin_original = "fn main() {\nprintln!(\"Hello, world!\");\n}";
        let bin_formatted = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let lib_file = "src/lib.rs";
        let lib_original = "fn greet() {\nprintln!(\"Greetings!\");\n}";
        let lib_formatted = "fn greet() {\n    println!(\"Greetings!\");\n}";

        let mut writer = Vec::new();
        let mut emitter = DiffEmitter::new(EmitterConfig {
            print_filename: true,
            ..EmitterConfig::default()
        });
        let _ = emitter
            .emit_formatted_file(
                &mut writer,
                FormattedFile {
                    filename: &FileName::Real(PathBuf::from(bin_file)),
                    original_text: bin_original,
                    formatted_text: bin_formatted,
                },
                NewlineStyle::default(),
            )
            .unwrap();
        let _ = emitter
            .emit_formatted_file(
                &mut writer,
                FormattedFile {
                    filename: &FileName::Real(PathBuf::from(lib_file)),
                    original_text: lib_original,
                    formatted_text: lib_formatted,
                },
                NewlineStyle::default(),
            )
            .unwrap();

        assert_eq!(
            String::from_utf8(writer).unwrap(),
            format!("{}\n{}\n", bin_file, lib_file),
        )
    }

    #[test]
    fn prints_newline_message_with_only_newline_style_diff() {
        let mut writer = Vec::new();
        let mut emitter = DiffEmitter::new(EmitterConfig::default());
        let _ = emitter
            .emit_formatted_file(
                &mut writer,
                FormattedFile {
                    filename: &FileName::Real(PathBuf::from("src/lib.rs")),
                    original_text: "fn empty() {}\n",
                    formatted_text: "fn empty() {}\r\n",
                },
                NewlineStyle::default(),
            )
            .unwrap();
        assert_eq!(
            String::from_utf8(writer).unwrap(),
            String::from("Incorrect newline style in src/lib.rs\n")
        );
    }
}
