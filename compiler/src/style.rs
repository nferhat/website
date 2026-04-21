//! Styling using [SASS](https://sass-lang.com/)
//!
//! In order to easily write styling, we include SASS as part of the pipeline for compiling
//! markdown content into HTML pages. This allows to easily define anything, and be done quickly
//! with tweaking the website.

use std::path::Path;

use codemap::SpanLoc;
use grass::Logger;

use crate::config;

/// Compiles the given input CSS into a proper stylesheet
pub fn compile_to_stylesheet(
    input: &str,
    load_paths: &[impl AsRef<Path>],
    syntax: config::StyleSyntax,
) -> Result<String, Box<grass::Error>> {
    let logger = TracingLogger {
        span: tracing::debug_span!("sass"),
    };
    let input_syntax = match syntax {
        config::StyleSyntax::Css => grass::InputSyntax::Css,
        config::StyleSyntax::Scss => grass::InputSyntax::Scss,
        config::StyleSyntax::Sass => grass::InputSyntax::Sass,
    };

    let options = grass::Options::default()
        .logger(&logger)
        .quiet(false)
        .style(grass::OutputStyle::Compressed)
        .load_paths(load_paths)
        .input_syntax(input_syntax);

    grass::from_string(input, &options)
}

#[derive(Debug)]
struct TracingLogger {
    span: tracing::Span,
}
impl Logger for TracingLogger {
    fn debug(&self, location: SpanLoc, message: &str) {
        let _span = self.span.enter();
        debug!(?location, "{message}");
        todo!()
    }

    fn warn(&self, location: SpanLoc, message: &str) {
        let _span = self.span.enter();
        warn!(?location, "{message}");
        todo!()
    }
}
