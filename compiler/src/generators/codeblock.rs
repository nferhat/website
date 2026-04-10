use markdown::{mdast, unist::Position};
use std::fmt::{self, Write};

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
