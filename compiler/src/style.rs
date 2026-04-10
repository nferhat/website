//! Styling using [SASS](https://sass-lang.com/)
//!
//! In order to easily write styling, we include SASS as part of the pipeline for compiling
//! markdown content into HTML pages. This allows to easily define anything, and be done quickly
//! with tweaking the website.

use codemap::SpanLoc;
use grass::Logger;

/// Compiles the given input CSS into a proper stylesheet
pub fn compile_to_stylesheet(input: &str) -> Result<String, Box<grass::Error>> {
    let logger = TracingLogger {
        span: tracing::debug_span!("sass"),
    };

    let options = grass::Options::default()
        .logger(&logger)
        .quiet(false)
        .style(grass::OutputStyle::Compressed)
        .input_syntax(grass::InputSyntax::Sass);

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
