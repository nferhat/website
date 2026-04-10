use std::fmt::{self, Write};

use crate::frontmatter::Frontmatter;

use super::{codeblock, heading};
use markdown::mdast;

fn prelude(title: impl AsRef<str>) -> String {
    let title = title.as_ref();
    format!(
        r#"<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>{title}</title>
        <!-- FIXME: Styling using compiled SASS -->
        <style>
            :root {{
                background-color: #141417;
                color: #cecece;
            }}

            .codeblock {{
                position: relative;
            }}
            .language-name {{
                font-family: monospace;
                position: absolute;
                padding: 6px;
                background-color: #cecece09;
                border-radius: 4px;
                right: 0;
                margin: 8px;
            }}

            pre {{
                padding: 12px;
                background-color: #101012;
                border-radius: 10px;
            }}
        </style>
    </head>

    <body><div>
"#
    )
}

fn epilogue() -> String {
    String::from("</div></body></html>")
}

/// Generates a simple blog page from the given frontmatter and markdown node tree.
///
/// The node is expected to be the [`mdast::Node::Root`] node, with it being traversed downwards.
/// It uses other [`generators`](super) in order to build the page.
pub fn generate(frontmatter: &Frontmatter, node: &mdast::Node) -> Result<String, fmt::Error> {
    let Some(children) = node.children() else {
        return Ok(String::new());
    };

    let mut buf = String::new();
    let out = &mut buf;
    writeln!(out, "{}", prelude(&frontmatter.title))?;

    for child in children {
        let position = child.position().unwrap().clone();
        match child {
            mdast::Node::Code(code) => {
                writeln!(out, "{}", codeblock::generate(position, code).unwrap())?;
            }
            mdast::Node::Heading(heading) => {
                let content =
                    heading::generate(position, heading.depth as usize, &heading.children);
                writeln!(out, "{}", content.unwrap())?;
            }
            mdast::Node::Text(text) => {
                writeln!(out, "<p>{}</p>", text.value)?;
            }
            _ => warn!(?child, "unhandled node"),
        }
    }

    writeln!(out, "{}", epilogue())?;

    Ok(buf)
}
