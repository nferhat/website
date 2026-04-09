use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use markdown::mdast;

use crate::frontmatter::Frontmatter;

mod frontmatter;

/// A single page on the site.
#[derive(Debug, Clone)]
pub struct SitePage {
    /// The (non-unique) name of this page.
    pub name: String,
    pub path: PathBuf,
    /// The frontmatter of this page.
    pub frontmatter: Frontmatter,

    /// The parsed markdown AST (Abstract Syntax Tree) of this page.
    ///
    /// NOTE: This still includes the node for the [`FrontmatterData`], you should skip it (it's the first node
    /// of this root node)
    pub contents: mdast::Node,
}

/// Walks through a given directory and parses all the `.md` files into [`SitePage`]s. This does not
/// do actual markdown parsing/compiling to HTML.
pub fn parse_content_dir(dir: &Path) -> Result<Vec<SitePage>, Error> {
    let mut res = vec![];
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    let iter = fs::read_dir(dir).map_err(Error::Io)?;
    for entry in iter {
        let entry = entry.map_err(Error::Io)?;
        // FIXME: This should not crash, right?
        let name = entry.file_name().into_string().unwrap();
        let path = entry.path();

        if path.is_dir() {
            res.extend(parse_content_dir(&path)?);
        } else if path.extension() == Some(OsStr::new("md")) {
            let file = fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(Error::Io)?;
            let raw_content = io::read_to_string(file).map_err(Error::Io)?;
            let contents =
                markdown::to_mdast(&raw_content, &parse_options()).map_err(Error::Markdown)?;

            // Get the frontmatter out.
            let children = contents.children().ok_or(Error::MissingFronmatter)?;
            let frontmatter_node = children.get(0).ok_or(Error::MissingFronmatter)?;
            let mdast::Node::Toml(mdast::Toml {
                value: frontmatter_str,
                ..
            }) = frontmatter_node
            else {
                return Err(Error::MissingFronmatter);
            };
            let frontmatter =
                frontmatter::from_str(frontmatter_str).map_err(Error::InvalidFrontmatter)?;

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
