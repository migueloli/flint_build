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
    #[error("Plugin '{plugin}': {message}")]
    Template { plugin: String, message: String },
    #[error("line {line}: field '{field}' of '{class}' {problem}. {fix}")]
    UnsupportedType {
        line: usize,
        class: String,
        field: String,
        problem: String,
        fix: &'static str,
    },
}

impl FlintError {
    /// A template error for `plugin`. Tera's own message is only the outermost line ("Failed to parse …"),
    /// so the whole `source()` chain is included to say what and where.
    pub fn template(plugin: &str, error: &tera::Error) -> Self {
        let mut message = error.to_string();
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            message.push_str(&format!(": {}", cause.to_string().trim()));
            source = cause.source();
        }
        FlintError::Template {
            plugin: plugin.to_string(),
            message,
        }
    }
}
