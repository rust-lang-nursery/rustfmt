use super::*;
use crate::emitter::EmitterConfig;
use std::fs;

#[derive(Debug, Default)]
pub struct FilesEmitter {
    print_misformatted_file_names: bool,
}

impl FilesEmitter {
    pub fn new(config: EmitterConfig) -> Self {
        Self {
            print_misformatted_file_names: config.print_filename,
        }
    }
}

impl Emitter for FilesEmitter {
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
        // Write text directly over original file if there is a diff.
        let filename = match filename {
            FileName::Stdin => return Err(EmitterError::InvalidInputForFiles),
            FileName::Real(path_buf) => path_buf,
        };
        if original_text != formatted_text {
            fs::write(filename, formatted_text)?;
            if self.print_misformatted_file_names {
                writeln!(output, "{}", filename.display())?;
            }
        }
        Ok(EmitterResult::default())
    }
}
