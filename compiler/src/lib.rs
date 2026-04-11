//! # `compiler` - main compiler for pages.
//!
//! This crate is responsible for reading markdown files and transforming them into HTML
//! with proper "templating" (more like just adding some styles, performing syntax highlighting,
//! etc...)

#[macro_use]
extern crate tracing;

pub mod frontmatter;
mod generator;
pub mod generators;
pub mod page;
mod style;

pub use generator::Generator;
pub use style::compile_to_stylesheet;

/// A result type that can be generated when compiling a site.
type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Formatting error: {0}")]
    Format(#[from] std::fmt::Error),
}
