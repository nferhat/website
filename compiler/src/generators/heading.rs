use std::fmt::{self, Write};

use markdown::{mdast, unist::Position};

pub fn generate(
    position: Position,
    level: usize,
    content: &[mdast::Node],
) -> Result<String, fmt::Error> {
    trace!(?position, %level, "Got heading");
    let mut buf = String::new();

    writeln!(&mut buf, "<h{level}>")?;

    for node in content {
        match node {
            mdast::Node::InlineCode(inline_code) => {
                write!(&mut buf, "<code>{}</code>", inline_code.value)?;
            }
            mdast::Node::Html(html) => write!(&mut buf, "{}", html.value)?,
            mdast::Node::Text(text) => write!(&mut buf, "{}", text.value)?,
            _ => unreachable!("Invalid expression in header"),
        }
    }

    writeln!(&mut buf, "</h{level}>")?;

    Ok(buf)
}
