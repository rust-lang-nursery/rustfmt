use std::io::Write;

use super::*;
use crate::emitter::{EmitterConfig, Verbosity};

#[derive(Debug)]
pub struct StdoutEmitter {
    verbosity: Verbosity,
}

impl StdoutEmitter {
    pub fn new(config: EmitterConfig) -> Self {
        Self {
            verbosity: config.verbosity,
        }
    }
}

impl Emitter for StdoutEmitter {
    fn emit_formatted_file(
        &mut self,
        output: &mut dyn Write,
        FormattedFile {
            filename,
            formatted_text,
            ..
        }: FormattedFile<'_>,
        _newline_style: NewlineStyle,
    ) -> Result<EmitterResult, EmitterError> {
        if self.verbosity != Verbosity::Quiet {
            writeln!(output, "{}:\n", filename)?;
        }
        write!(output, "{}", formatted_text)?;
        Ok(EmitterResult::default())
    }
}
