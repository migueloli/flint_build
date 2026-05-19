use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlintError {
    #[error("Syntax Error in {file} at line {line}, column {column}\n\n{source_line}\n{pointer}\n")]
    Syntax {
        file: String,
        line: usize,
        column: usize,
        source_line: String,
        pointer: String,
    },
}
