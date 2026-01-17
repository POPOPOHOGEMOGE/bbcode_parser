use thiserror::Error;

#[derive(Debug, Error)]
pub enum BbCodeError {
    #[error("Input size exceeded limit (max {max_size} bytes)")]
    InputSizeExceeded { max_size: usize, actual_size: usize },

    #[error("Parsed tag count exceeded limit (max {max_tags})")]
    TagCountExceeded { max_tags: usize },

    #[error("Nest depth exceeded limit (max {max_depth}). Near: \"{near}\"")]
    NestDepthExceeded { max_depth: usize, near: String },

    #[error("Failed to parse input: {0}")]
    PestError(#[from] pest::error::Error<crate::parser::Rule>),
}
