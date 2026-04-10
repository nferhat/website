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
    stylesheet_path: Option<String>,
    /// Whether this sitepage has math.
    ///
    /// If `true`, we need to include a special math script from [MathJax](https://mathjax.org)
    /// in order to properly handle/render math.
    has_math: bool,
}

impl Generator {
    /// Creates a new generator for a given [`SitePage`].
    ///
    /// The `stylesheet` parameter will be the URL/path from where the page will load it's stylesheet.
    /// Essentially, it's the `href` parameter of a `<link rel="stylesheet"> tag.
    pub fn new(page: SitePage, stylesheet: Option<impl Into<String>>) -> Self {
        Self {
            page,
            stylesheet_path: stylesheet.map(Into::into),
            has_math: false,
        }
    }

    pub fn html(mut self) -> Result<String> {
        // FIX: This is currently only a blog generator
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

        // We first need to generate the HTML for the children since we need to determine whether
        // the content has math in order to only include mathjax when needed.
        let mut body = String::new();
        for child in root.children.drain(..) {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, &mut body)?;
        }

        let out = format!(
            "{prelude}{body}{epilogue}",
            prelude = blog::prelude(
                &frontmatter.title,
                self.stylesheet_path.as_ref().map(String::as_str),
                self.has_math,
            ),
            epilogue = blog::epilogue()
        );

        Ok(out)
    }

    fn generate_common(
        &mut self,
        node: mdast::Node,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
        match node {
            mdast::Node::Blockquote(blockquote) => {
                self.generate_node_with_children(
                    "blockquote",
                    &[],
                    blockquote.children.into_iter(),
                    out,
                )?;
            }

            mdast::Node::Break(_) => write!(out, "<br>")?,
            mdast::Node::InlineCode(inline_code) => {
                self.generate_inline_code(inline_code, position, out)?
            }
            mdast::Node::InlineMath(inline_math) => {
                self.has_math = true;
                write!(
                    out,
                    "<span class=\"math-inline\">\\({}\\)</p>",
                    inline_math.value
                )?;
            }
            mdast::Node::Math(math) => {
                self.has_math = true;
                write!(out, "<span class=math>\\[{}\\]</span>", math.value)?;
            }
            mdast::Node::Delete(delete) => self.generate_strikethrough(delete, position, out)?,
            mdast::Node::Emphasis(emphasis) => {
                self.generate_node_with_children("em", &[], emphasis.children.into_iter(), out)?;
            }
            mdast::Node::Html(html) => write!(out, "{}", html.value)?,
            mdast::Node::Strong(strong) => {
                self.generate_node_with_children("strong", &[], strong.children.into_iter(), out)?;
            }
            mdast::Node::Text(text) => write!(out, "{}", text.value)?,
            mdast::Node::Code(code) => self.generate_codeblock(code, position, out)?,
            mdast::Node::Heading(heading) => self.generate_heading(heading, position, out)?,
            mdast::Node::ThematicBreak(_) => write!(out, "<hr>")?,
            mdast::Node::Paragraph(paragraph) => {
                self.generate_node_with_children("p", &[], paragraph.children.into_iter(), out)?;
            }

            // FIX: images
            mdast::Node::Image(_image) => todo!(),
            mdast::Node::ImageReference(_image_reference) => todo!(),
            // FIX: Links
            mdast::Node::Link(_link) => todo!(),
            mdast::Node::LinkReference(_link_reference) => todo!(),
            mdast::Node::Definition(_definition) => todo!(),
            // FIX: Tables
            mdast::Node::Table(_table) => todo!(),
            mdast::Node::TableRow(_table_row) => todo!(),
            mdast::Node::TableCell(_table_cell) => todo!(),
            // FIX: Footnotes
            mdast::Node::FootnoteDefinition(_footnote_definition) => todo!(),
            mdast::Node::FootnoteReference(_footnote_reference) => todo!(),
            // FIX: Lists
            mdast::Node::List(_list) => todo!(),
            mdast::Node::ListItem(_list_item) => todo!(),

            _ => unreachable!("MDX is disabled"),
        }

        Ok(())
    }

    fn generate_heading(
        &mut self,
        heading: mdast::Heading,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
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
    ) -> Result {
        trace!(?position, "Got inline code");
        write!(out, "<code class=inline-code>{}</code>", inline_code.value)?;
        Ok(())
    }

    fn generate_strikethrough(
        &mut self,
        delete: mdast::Delete,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
        trace!(?position, "Got delete/strikethrough");
        write!(out, "<del>")?;
        for child in delete.children {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, out)?;
        }
        write!(out, "</del>")?;
        Ok(())
    }

    fn generate_codeblock(
        &mut self,
        code: mdast::Code,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
        let res = codeblock::generate(position, &code)?;
        write!(out, "{res}")?;
        Ok(())
    }

    fn generate_node_with_children(
        &mut self,
        tag_name: &str,
        classes: &[&str],
        children: impl IntoIterator<Item = mdast::Node>,
        out: &mut impl Write,
    ) -> Result {
        write!(out, "<{tag_name} ")?;
        if classes.len() != 0 {
            write!(out, "class=\"")?;
            for class in classes {
                write!(out, "{class} ")?;
            }
            write!(out, "\"")?;
        }
        write!(out, ">")?;

        for child in children {
            let position = child.position().cloned().unwrap();
            self.generate_common(child, position, out)?;
        }

        write!(out, "</{tag_name}>")?;

        Ok(())
    }
}

/// A result type that can be generated by the [`Generator`]
type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Formatting error: {0}")]
    Format(#[from] fmt::Error),
}
