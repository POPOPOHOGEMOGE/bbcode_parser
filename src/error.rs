use crate::ast::Span;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BbCodeError {
    #[error("Input size exceeded limit (max {max_size} bytes)")]
    InputSizeExceeded { max_size: usize, actual_size: usize },

    #[error("Parsed tag count exceeded limit (max {max_tags})")]
    TagCountExceeded { max_tags: usize },

    #[error(
        "Nest depth exceeded limit (max {max_depth}) at line {line}, col {column}. Near: \"{near}\""
    )]
    NestDepthExceeded {
        max_depth: usize,
        near: String,
        span: Span,
        line: usize,
        column: usize,
    },

    #[error("Failed to parse input: {0}")]
    PestError(#[from] pest::error::Error<crate::parser::Rule>),
}
