pub mod ast;
pub mod error;
pub mod options;
pub mod registry;

pub mod parser;
pub mod render;

pub use ast::{Element, Node};
pub use error::BbCodeError;
pub use options::BbCodeOptions;

pub use parser::parse_bbcode_to_ast;
pub use render::ast_to_html;

pub fn bbcode_to_html(input: &str, opts: &BbCodeOptions) -> Result<String, BbCodeError> {
    let ast = parse_bbcode_to_ast(input, opts)?;
    Ok(ast_to_html(&ast))
}
