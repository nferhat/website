use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use markdown::mdast;

use crate::frontmatter::{self, Frontmatter};

/// A single page on the site.
///
/// A page represents a single "translation unit" (if you were trying to understand this
/// project with a true compiler sense, this would be a single `.o` file). The `frontmatter`
/// of the page determines which base "template" surrounds the content.
///
/// The actual content is pased using [GitHub-Flavoured Markdown](https://github.github.com/gfm/),
/// which provides a lot
#[derive(Debug, Clone)]
pub struct SitePage {
    /// The (non-unique) name of this page.
    pub name: String,
    /// The unique path of the markdown file associated with this page.
    ///
    /// This path is used to derive the name of the generated HTML file.
    pub path: PathBuf,
    /// The frontmatter of this page.
    pub frontmatter: Frontmatter,

    /// The parsed markdown AST (Abstract Syntax Tree) of this page.
    ///
    /// NOTE: This still includes the node for the [`FrontmatterData`], you should skip it (it's the first node
    /// of this root node)
    pub contents: mdast::Node,
}

/// Options used to do the parsing of the config.
///
/// It's GFM+some additional stuff enabled.
fn parse_options() -> markdown::ParseOptions {
    markdown::ParseOptions {
        constructs: markdown::Constructs {
            autolink: true,
            code_text: true,
            gfm_autolink_literal: true,
            html_flow: true,
            label_start_image: true,
            math_flow: true,
            block_quote: true,
            frontmatter: true,
            ..markdown::Constructs::gfm()
        },
        ..Default::default()
    }
}
/// Finds all the pages in a given directory.
///
/// This recursively walks down in the directory, searching for markdown files with valid frontmatter.
pub fn get_pages(path: impl AsRef<Path>) -> Result<Vec<SitePage>, Error> {
    let path = path.as_ref();
    let mut res = vec![];
    if !path.is_dir() {
        debug!(?path, "Skipping path since it's not a directory");
        return Ok(vec![]);
    }

    let iter = fs::read_dir(path).map_err(Error::Io)?;
    for entry in iter {
        let entry = entry.map_err(Error::Io)?;
        // FIXME: This should not crash, right?
        let name = entry.file_name().into_string().unwrap();
        let path = entry.path();

        if path.is_dir() {
            res.extend(get_pages(&path)?);
        } else if path.extension() == Some(OsStr::new("md")) {
            let file = fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(Error::Io)?;
            let raw_content = io::read_to_string(file).map_err(Error::Io)?;
            let mut contents =
                markdown::to_mdast(&raw_content, &parse_options()).map_err(Error::Markdown)?;

            // Get the frontmatter out.
            let children = contents.children_mut().ok_or(Error::MissingFronmatter)?;
            if children.len() < 1 {
                warn!(?name, "Skipping file without frontmatter");
                return Err(Error::MissingFronmatter);
            }
            let frontmatter_node = children.remove(0);
            let mdast::Node::Toml(mdast::Toml {
                value: frontmatter_str,
                ..
            }) = frontmatter_node
            else {
                return Err(Error::MissingFronmatter);
            };
            let frontmatter =
                frontmatter::from_str(&frontmatter_str).map_err(Error::InvalidFrontmatter)?;

            res.push(SitePage {
                name,
                path,
                frontmatter,
                contents,
            });
        }
    }

    Ok(res)
}

/// Error than can occur while parsing a site page.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Missing frontmatter")]
    MissingFronmatter,
    #[error("Invalid frontmatter data: {0}")]
    InvalidFrontmatter(toml::de::Error),
    #[error("I/O error: {0}")]
    Io(io::Error),
    #[error("Error parsing markdown: {0}")]
    Markdown(markdown::message::Message),
}
