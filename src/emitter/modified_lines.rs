use super::*;
use rustfmt_diff::{make_diff, ModifiedLines};

#[derive(Debug, Default)]
pub struct ModifiedLinesEmitter;

impl Emitter for ModifiedLinesEmitter {
    fn emit_formatted_file(
        &mut self,
        output: &mut dyn Write,
        FormattedFile {
            original_text,
            formatted_text,
            ..
        }: FormattedFile<'_>,
        _newline_style: NewlineStyle,
    ) -> Result<EmitterResult, EmitterError> {
        const CONTEXT_SIZE: usize = 0;
        let mismatch = make_diff(original_text, formatted_text, CONTEXT_SIZE);
        let has_diff = !mismatch.is_empty();
        write!(output, "{}", ModifiedLines::from(mismatch))?;
        Ok(EmitterResult { has_diff })
    }
}
