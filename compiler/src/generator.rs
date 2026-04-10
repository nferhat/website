//! A generator for a single [`SitePage`](super::page::SitePage).
//!
//! A central state structure is needed when generating a page, in order to keep track of different
//! things such as:
//!
//! 1. Links and make sure they are valid (at least links to other parts of the page)
//! 2. All headers of this page, in order to generate a TOC
//! 3. Footnotes, since they are basically a pretty hashmap with links for going back and forth.
//! 4. Other misc stuff, for better logging.

// TODO: Link support
// TODO: Table of contents(TOC)
// TODO: Footnotes

use fmt::Write;
use std::fmt;

use markdown::{mdast, unist::Position};

use crate::{
    generators::{blog, codeblock},
    page::SitePage,
};

pub struct Generator {
    page: SitePage,
}

impl Generator {
    pub fn new(page: SitePage) -> Self {
        Self { page }
    }

    pub fn html(mut self) -> Result<String, Error> {
        // FIX: This is currently only a blog generator
        let mut buffer = String::new();
        let SitePage {
            name,
            path,
            contents,
            frontmatter,
        } = &self.page;
        let frontmatter = frontmatter.clone();
        info!(%name, ?path, "Generating HTML for page");

        // FIX: Clone since we use self methods in the generator loop.
        let mdast::Node::Root(mut root) = contents.clone() else {
            unreachable!()
        };

        let out = &mut buffer;
        writeln!(out, "{}", blog::prelude(&frontmatter.title))?;

        for child in root.children.drain(..) {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, out)?;
        }

        writeln!(out, "{}", blog::epilogue())?;

        Ok(buffer)
    }

    fn generate_common(
        &mut self,
        node: mdast::Node,
        position: Position,
        out: &mut impl Write,
    ) -> fmt::Result {
        match node {
            mdast::Node::Blockquote(blockquote) => {
                write!(out, "<blockquote>")?;
                for child in blockquote.children {
                    let position = child.position().unwrap().clone();
                    self.generate_common(child, position, out)?;
                }
                write!(out, "</blockquote>")?;
            }
            // FIX: Footnotes
            mdast::Node::FootnoteDefinition(footnote_definition) => todo!(),
            // FIX: Lists
            mdast::Node::List(list) => todo!(),
            mdast::Node::Break(_) => write!(out, "<br>")?,
            mdast::Node::InlineCode(inline_code) => {
                self.generate_inline_code(inline_code, position, out)?;
            }
            // FIX: Math
            mdast::Node::InlineMath(inline_math) => todo!(),
            mdast::Node::Delete(delete) => todo!(),
            mdast::Node::Emphasis(emphasis) => {
                write!(out, "<em>")?;
                for child in emphasis.children {
                    let position = child.position().unwrap().clone();
                    self.generate_common(child, position, out)?;
                }
                write!(out, "</em>")?;
            }
            mdast::Node::FootnoteReference(footnote_reference) => todo!(),
            mdast::Node::Html(html) => todo!(),
            // FIX: images
            mdast::Node::Image(image) => todo!(),
            mdast::Node::ImageReference(image_reference) => todo!(),
            mdast::Node::Link(link) => todo!(),
            mdast::Node::LinkReference(link_reference) => todo!(),
            mdast::Node::Strong(strong) => todo!(),
            mdast::Node::Text(text) => write!(out, "{}", text.value)?,
            mdast::Node::Code(code) => self.generate_codeblock(code, position, out)?,
            // FIX: Math
            mdast::Node::Math(math) => todo!(),
            mdast::Node::MdxFlowExpression(mdx_flow_expression) => todo!(),
            mdast::Node::Heading(heading) => self.generate_heading(heading, position, out)?,
            // FIX: Tables
            mdast::Node::Table(table) => todo!(),
            mdast::Node::TableRow(table_row) => todo!(),
            mdast::Node::TableCell(table_cell) => todo!(),
            mdast::Node::ThematicBreak(_) => write!(out, "<hr>")?,
            // FIX: List items
            mdast::Node::ListItem(list_item) => todo!(),
            // FIX: Definitions
            mdast::Node::Definition(definition) => todo!(),
            mdast::Node::Paragraph(paragraph) => {
                write!(out, "<p>")?;
                for child in paragraph.children {
                    let position = child.position().unwrap().clone();
                    self.generate_common(child, position, out)?;
                }
                write!(out, "</p>")?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn generate_heading(
        &mut self,
        heading: mdast::Heading,
        position: Position,
        out: &mut impl Write,
    ) -> fmt::Result {
        trace!(?position, level = heading.depth, "Got heading");

        // NOTE: Here we limit what we can render inside a heading, otherwise other pieces of
        // code should instead delegate this work to self.generate_common()
        write!(out, "<h{}>", heading.depth)?;
        for child in heading.children {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, out)?;
        }
        write!(out, "</h{}>", heading.depth)?;

        Ok(())
    }

    fn generate_inline_code(
        &mut self,
        inline_code: mdast::InlineCode,
        position: Position,
        out: &mut impl Write,
    ) -> fmt::Result {
        trace!(?position, "Got inline code");
        write!(out, "<code>{}</code>", inline_code.value)?;
        Ok(())
    }

    fn generate_strikethrough(
        &mut self,
        delete: mdast::Delete,
        position: Position,
        out: &mut impl Write,
    ) -> fmt::Result {
        trace!(?position, "Got delete/strikethrough");
        write!(out, "<del>")?;
        for child in delete.children {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, out);
        }
        write!(out, "</del>")?;
        Ok(())
    }

    fn generate_codeblock(
        &mut self,
        code: mdast::Code,
        position: Position,
        out: &mut impl Write,
    ) -> fmt::Result {
        let res = codeblock::generate(position, &code)?;
        write!(out, "{res}")?;
        Ok(())
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Formatting error: {0}")]
    Format(#[from] fmt::Error),
}
