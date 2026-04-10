use markdown::{mdast, unist::Position};
use std::fmt::{self, Write};

// TODO: Tree-sitter highlighting in codeblocks.
//
// I already checked out the tree-sitter's CLI code and some other things (like tree_sitter_highlighter).
// It seems like they already have an integrated Html "renderer" which turns the code into spans automatically
// (as long as you provide it with a language and syntax queries)
//
// Speaking of which, other questions arise:
//
// 1. How should languages be bundled with the tool? I think I'm gonna use the repository of Zed's languages
//    since it's gonna make my life simpler.
//
// 2. Should each "website" (even though this tool is only meant for one) bundle their own parsers? This would
//    allow for local overrides and such. Could be an interesting path to explore.
//
// 3. How should we handle custom queries? This is useful for specific stuff (when trying to highlight something
//    to show off in a codeblock)

fn codeblock_prelude(lang_name: impl AsRef<str>) -> String {
    let lang_name = lang_name.as_ref();
    format!(
        r#"<div class="codeblock">
    <p class="language-name">{lang_name}</p>
    <code><pre>"#
    )
}
static CODEBLOCK_EPILOGUE: &str = "</code></pre></div>";

pub fn generate(position: Position, code: &mdast::Code) -> Result<String, fmt::Error> {
    trace!(?position, "Got codeblock");
    let lang = code.lang.clone().unwrap_or_else(|| String::from("unknown"));

    let mut buf = String::new();
    let out = &mut buf;

    write!(out, "{}", codeblock_prelude(lang))?;

    let code_str = html_escape::encode_text(&code.value);
    write!(out, "{}", code_str)?;

    write!(out, "{CODEBLOCK_EPILOGUE}")?;

    Ok(buf)
}
