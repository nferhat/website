//! A generator for a site content.
//!
//! This is strictly for the body/content, IE transforming markdown nodes into actual other nodes.
//! This does not generate a full page.

// TODO: Table of contents(TOC)

use fmt::Write;
use std::fmt;
use url::Url;

use markdown::{
    mdast::{self, Node},
    unist::Position,
};

use crate::{Result, generators::codeblock};

pub struct Generator {
    root_node: Node,
    /// Whether this sitepage has math.
    ///
    /// If `true`, we need to include a special math script from [MathJax](https://mathjax.org)
    /// in order to properly handle/render math.
    pub has_math: bool,
}

impl Generator {
    /// Creates a new generator for a given [`SitePage`].
    ///
    /// The `stylesheet` parameter will be the URL/path from where the page will load it's stylesheet.
    /// Essentially, it's the `href` parameter of a `<link rel="stylesheet"> tag.
    pub fn new(root_node: Node) -> Self {
        Self {
            root_node,
            has_math: false,
        }
    }

    pub fn to_html(mut self) -> Result<String> {
        // FIX: Clone since we use self methods in the generator loop.
        let mdast::Node::Root(mut root) = self.root_node.clone() else {
            unreachable!()
        };

        // NOTE: Before there was a "pre-pass" here in order to ensure correct links and whatnot.
        // But it turns out the [`markdown`] crate already handles this for us and doesn't give us
        // references/footnotes to stuff that doesn't exist, and instead just emits Text nodes.

        // We first need to generate the HTML for the children since we need to determine whether
        // the content has math in order to only include mathjax when needed.
        let mut body = String::new();

        // FIXME: Move to compiler
        // // Before children, we generate a little paragraph with metadata with the title
        // writeln!(
        //     &mut body,
        //     "<h1 class={class}>{txt}</h1>",
        //     class = if !frontmatter.metadata {
        //         "pad-down"
        //     } else {
        //         ""
        //     },
        //     txt = frontmatter.title
        // )?;

        // if frontmatter.metadata {
        //     let word_count = count_words(&contents);
        //     let reading_speed = 175.0; // FIX: Not hardcode
        //     let (minutes, seconds) = estimate_reading_time_min_sec(word_count, reading_speed);
        //     write!(
        //         &mut body,
        //         "<p class=metadata>{word_count} words &bull; {minutes}'{seconds}\""
        //     )?;
        //     if frontmatter.tags.len() >= 1 {
        //         for tag in &frontmatter.tags {
        //             write!(&mut body, " &bull; #{tag}")?;
        //         }
        //     }
        //     writeln!(&mut body, "</p>")?;
        // }

        for child in root.children.drain(..) {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, &mut body)?;
        }

        Ok(body)
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
            mdast::Node::ThematicBreak(_) => write!(out, "<hr>")?,

            mdast::Node::InlineCode(inline_code) => {
                self.generate_inline_code(inline_code, position, out)?
            }
            mdast::Node::Code(code) => self.generate_codeblock(code, position, out)?,
            mdast::Node::Delete(delete) => self.generate_strikethrough(delete, position, out)?,
            mdast::Node::Emphasis(emphasis) => {
                self.generate_node_with_children("em", &[], emphasis.children.into_iter(), out)?;
            }
            mdast::Node::Html(html) => write!(out, "{}", html.value)?,
            mdast::Node::Strong(strong) => {
                self.generate_node_with_children("strong", &[], strong.children.into_iter(), out)?;
            }
            mdast::Node::Text(text) => write!(out, "{}", text.value)?,
            mdast::Node::Heading(heading) => self.generate_heading(heading, position, out)?,
            mdast::Node::Paragraph(paragraph) => {
                self.generate_node_with_children("p", &[], paragraph.children.into_iter(), out)?;
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

            mdast::Node::Definition(_definition) => {
                // unreachable!("link definitions are handled in the pre-pass")
            }
            mdast::Node::Link(link) => {
                self.generate_link(link, position, out)?;
            }
            mdast::Node::LinkReference(_link_reference) => {
                unimplemented!("link references are not implemented, just use basic links!")
            }

            mdast::Node::Image(image) => {
                self.generate_image(image, position, out)?;
            }
            mdast::Node::ImageReference(_image_reference) => {
                unimplemented!("image references are not implemented, just use basic images!")
            }

            mdast::Node::FootnoteDefinition(def) => {
                write!(
                    out,
                    r#"<span class=footnote id="footnote-{}">"#,
                    def.identifier
                )?;
                write!(
                    out,
                    r#"<span class=footnote-id>({})</span>"#,
                    def.identifier
                )?;

                for child in def.children {
                    let position = child.position().unwrap().clone();
                    self.generate_common(child, position, out)?;
                }

                writeln!(
                    out,
                    r##"<a class=footnote-back href="#footnote-back-{}">&#8617</a>"##,
                    def.identifier
                )?;
                write!(out, "</span>")?;
            }
            mdast::Node::FootnoteReference(fref) => {
                let mdast::FootnoteReference { identifier, .. } = fref;
                // This trick is from <https://stackoverflow.com/questions/66964/how-do-i-create-a-link-to-a-footnote-in-html>
                // We create a set of anchors, one to go down to the footnote, and one to go back.
                write!(
                    out,
                    r##"<a class="footnote-ref" id="footnote-back-{identifier}" href="#footnote-{identifier}"><sup>{identifier}</sup></a>"##
                )?;
            }

            mdast::Node::List(list) => {
                let tag = if list.ordered { "ol" } else { "ul" };
                self.generate_node_with_children(tag, &[], list.children, out)?;
            }
            mdast::Node::ListItem(list_item) => {
                self.generate_node_with_children("li", &[], list_item.children, out)?;
            }

            // FIX: Tables
            mdast::Node::Table(_table) => todo!(),
            mdast::Node::TableRow(_table_row) => todo!(),
            mdast::Node::TableCell(_table_cell) => todo!(),

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
        trace!(?position, "Got strikethrough/delete");
        self.generate_node_with_children("span", &["strikethrough"], delete.children, out)
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

    fn generate_link(
        &mut self,
        link: mdast::Link,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
        trace!(?position, "Got link");

        write!(out, "<a ")?;
        if let Some(title) = link.title {
            write!(out, "title=\"{title}\" ")?;
        }

        // Now, depending on whether the given link is an url or not, we try to find the reference.
        if Url::parse(&link.url).is_ok() {
            write!(out, "href=\"{}\"", link.url)?;
        } else {
            // Otherwise, try to search for existing references.
            // We are assured that all the link definitions of the document are here since we do a prepass.

            // if let Some(url) = self.link_defs.get(&link.url) {
            //     write!(out, "href=\"{}\"", url)?;
            // } else {
            //     self.invalid_links.insert(link.url.clone());
            // };

            // FIX: I simplified the generator here
        }

        write!(out, ">")?;

        for child in link.children {
            let position = child.position().unwrap().clone();
            self.generate_common(child, position, out)?;
        }

        write!(out, "</a>")?;

        Ok(())
    }

    // Basically the same as generate_link
    fn generate_image(
        &mut self,
        image: mdast::Image,
        position: Position,
        out: &mut impl Write,
    ) -> Result {
        trace!(?position, "Got image");

        // Additional div to make a cool popout effect using css
        write!(
            out,
            r#"<div class=image-container><img alt="{}" "#,
            image.alt
        )?;
        if let Some(title) = &image.title {
            write!(out, r#"title="{}" "#, title)?;
        }

        // Now, depending on whether the given link is an url or not, we try to find the reference.
        if Url::parse(&image.url).is_ok() {
            write!(out, "src=\"{}\"", image.url)?;
        } else {
            // Otherwise, try to search for existing references. The same as link references.
            // This allow for flexibility in case the user wants (or not) to show the image as content
            // or just link to it.

            // if let Some(url) = self.link_defs.get(&image.url) {
            //     write!(out, "src=\"{}\"", url)?;
            // } else {
            //     self.invalid_links.insert(image.url.clone());
            // };

            // FIX: I simplified things here
        }
        write!(out, ">")?;

        if let Some(title) = &image.title {
            write!(out, r#"<span class=image-title>{}</span>"#, title)?;
        }

        write!(out, "</div>")?;

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
