//! # `compiler` - main compiler for pages.
//!
//! This crate is responsible for reading markdown files and transforming them into HTML
//! with proper "templating" (more like just adding some styles, performing syntax highlighting,
//! etc...)

#[macro_use]
extern crate tracing;

mod config;
mod frontmatter;
mod generator;
mod style;
mod templates;
mod utils;

use std::{
    collections::{HashMap, hash_map},
    path::{self, Path, PathBuf},
    sync::Arc,
};

use eyre::{Context, bail, eyre};
use pulldown_cmark::{Event, MetadataBlockKind, Options, Tag, TagEnd};
use tokio::{fs, io};
use utils::to_dot_relative;

pub use config::{BlogConfig, Config, Error as ConfigError, StylingConfig};
pub use generator::write_html_fmt;
pub use style::compile_to_stylesheet;

use templates::{Templates, context};

use crate::utils::strip_leading_segment;

/// The main compiler.
pub struct Compiler {
    /// The root of the website. All paths inside the configuration and such will be
    /// relative to this path.
    root: PathBuf,
    /// Website configuration
    config: Arc<Config>,
    // Pre-cache some paths here because we are going to use them a lot.
    #[allow(unused)]
    build_path: PathBuf,
    style_output_path: PathBuf,

    /// The templates for this website.
    templates: Templates,

    /// The cached pages of this site.
    ///
    /// Pages are cached by their original file name, stripped of the content prefix, of course.
    ///
    /// For example, the page generated from `./content/blog/i-love-oranges.md` would be cached under
    /// the key `blog/i-love-oranges.md`.
    ///
    /// [`&BlogPage`](context::BlogPage) can easily be transformed into a [`Page`](context::Page),
    /// so it can be passed into the templating engine quickly.
    page_cache: HashMap<String, SitePage>,

    /// The cached [`Site`](context::Site) context passed into the templates.
    site_ctx: context::Site,
    /// The cached [`Build`](context::Build) context passed into the templates.
    build_ctx: context::Build,
}

impl Compiler {
    pub async fn new(root: impl AsRef<Path>, config: &Arc<Config>) -> eyre::Result<Self> {
        let root = path::absolute(root)?;

        let build_path = root.join("dist");
        let style_output_path = build_path.join("style.css");

        let templates_dir = root.join("templates");
        let templates = Templates::new(&templates_dir).await?;

        // Create and cache contexts here.
        let config = Arc::clone(config);
        let site_ctx = context::Site {
            base_url: config.base_url.clone(),
        };
        let build_ctx = context::Build { dev: false };

        let mut this = Self {
            root,
            build_path,
            style_output_path,
            templates,
            config,
            site_ctx,
            build_ctx,
            page_cache: HashMap::new(),
        };

        // Run a first compile pass in order to populate the page cache.
        this.compile_all()
            .await
            .context("failed to run initial compilation")?;

        Ok(this)
    }

    pub fn set_dev_mode(&mut self, dev_mode: bool) {
        self.build_ctx.dev = dev_mode;
    }

    /// Do a full-pass compile.
    ///
    /// This goes over all the pages of the website and generates everything. This also cleans up
    /// any cached page from incremental compilation.
    pub async fn compile_all(&mut self) -> eyre::Result<()> {
        self.recompile_stylesheets().await?;

        // let root = Arc::clone(&self;
        self.recompile_pages(self.root.clone()).await?;

        Ok(())
    }

    async fn recompile_pages(&mut self, dir: impl AsRef<Path>) -> eyre::Result<()> {
        let mut stack = Vec::with_capacity(10);
        stack.push(dir.as_ref().to_owned());

        while let Some(dir) = stack.pop() {
            let mut read_dir = fs::read_dir(dir)
                .await
                .context("failed to read directory")?;
            'entries: while let Some(entry) = read_dir.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    // push it onto the stack and continue with the next entry
                    stack.push(path);
                    continue 'entries;
                } else {
                    let path_str = path.to_string_lossy();
                    if !path_str.contains(&self.config.content_dir) {
                        // We are only interested in content.
                        continue 'entries;
                    }

                    self.recompile_page(path).await?;
                }
            }
        }

        Ok(())
    }

    /// Recompiles all stylesheets.
    ///
    /// Sadly since stylesheets can include eachother and we are outsourcing the compilation process
    /// to a 3rd-party library [`grass`], we cannot reliably do finer incremental compilation of styles.
    ///
    /// Plus, CSS composes over multiple files, sooooo...
    pub async fn recompile_stylesheets(&mut self) -> eyre::Result<()> {
        let style_input = self.root.join(&self.config.styling.root_file);
        // include the file's parent directory in the import paths
        let import_path = style_input.parent().map(ToOwned::to_owned).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "unable to find stylesheet parent")
        })?;
        let sass_input = fs::read_to_string(&style_input).await?;
        // and also what the user wants us to import
        let mut load_paths = self.config.styling.load_paths.clone();
        load_paths.push(import_path);
        let syntax = self.config.styling.syntax;

        // asyncify the compiling process, since it might read from other files on the filesystem and such
        // I don't know if this even helps but who am I to talk
        let style_res = tokio::task::spawn_blocking(move || {
            style::compile_to_stylesheet(&sass_input, &load_paths, syntax)
        });

        let style_contents = match style_res.await {
            Ok(res) => res.context("sass compilation error"),
            Err(_) => eyre::bail!("background task failed"),
        }?;

        fs::write(&self.style_output_path, style_contents).await?;

        Ok(())
    }

    /// Does a partial recompilation of the given `input_path`.
    ///
    /// It figures out the needed other pages that need to recompile in extra to this one,
    /// for example if you ask to recompile a blog page, the blogs index page will also update.
    pub async fn recompile_page(&mut self, input_path: impl AsRef<Path>) -> eyre::Result<()> {
        let input_path = to_dot_relative(input_path);

        let file_contents = fs::read_to_string(&input_path).await?;

        let options = Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_SMART_PUNCTUATION
            | Options::ENABLE_HEADING_ATTRIBUTES
            | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
            | Options::ENABLE_OLD_FOOTNOTES
            | Options::ENABLE_MATH
            | Options::ENABLE_SUPERSCRIPT
            | Options::ENABLE_SUBSCRIPT;
        let mut parser = pulldown_cmark::Parser::new_ext(&file_contents, options);

        // First extract the frontmatter from the parser
        let frontmatter = get_frontmatter(&mut parser)?;

        let body = {
            let mut out = String::with_capacity(1024);
            write_html_fmt(&mut out, parser)?;
            out
        };

        let content_path = strip_leading_segment(&input_path, "content");
        // FIXME: to_string_lossy may create duplicate keys in the `page_cache`
        let content_path_str = content_path.to_string_lossy().to_string();
        let output_path = self.build_path.join(&content_path).with_extension("html");
        // We can calculate the resulting URL from the output path.
        let url = content_path.with_extension("html");
        let url = url.to_string_lossy();

        let filename = input_path.file_name().unwrap().to_string_lossy();
        let slug = {
            let slug = input_path.file_stem().unwrap();
            slug.to_string_lossy()
        };

        // Meta information.
        const WPM: f64 = 175.0;
        let word_count = 1000; // WIP:
        let reading_time = ((word_count as f64 / WPM) * 60.0).round() as usize;

        let blog_page = match self.page_cache.entry(content_path_str.clone()) {
            hash_map::Entry::Occupied(mut occupied_entry) => {
                *occupied_entry.get_mut() = SitePage {
                    // NOTE: It is even worth it to cache the body here?
                    contents: body.clone(),
                    filename: filename.to_string(),
                    slug: slug.to_string(),
                    url: url.to_string(),
                    tags: frontmatter.tags.clone(),
                    title: frontmatter.title.clone(),
                    draft: frontmatter.draft,
                    meta: context::PageMeta {
                        reading_time,
                        word_count,
                    },
                };
                &*occupied_entry.into_mut()
            }
            hash_map::Entry::Vacant(vacant_entry) => {
                let blog_page = SitePage {
                    // NOTE: It is even worth it to cache the body here?
                    contents: body.clone(),
                    filename: filename.to_string(),
                    slug: slug.to_string(),
                    url: url.to_string(),
                    tags: frontmatter.tags.clone(),
                    title: frontmatter.title.clone(),
                    draft: frontmatter.draft,
                    meta: context::PageMeta {
                        reading_time,
                        word_count,
                    },
                };
                &*vacant_entry.insert(blog_page)
            }
        };

        let context = context::Context {
            page: (blog_page).into(),
            site: &self.site_ctx,
            build: &self.build_ctx,
        };

        let template = match &frontmatter.template {
            Some(name) => {
                if let Ok(requested) = self.templates.get_template(name).await {
                    requested
                } else {
                    self.templates.index_template()
                }
            }
            None => self.templates.index_template(),
        };

        let context = liquid::to_object(&context)?;
        let content = template.render(&context)?;
        trace!(?output_path, "Writing HTML");
        if let Some(parent) = output_path.parent() {
            trace!(?output_path, "Creating parent directory");
            fs::create_dir_all(parent)
                .await
                .context("failed to create parent directory")?;
        }

        fs::write(&output_path, content).await?;

        Ok(())
    }

    /// Reloads a given template path.
    pub async fn reload_template(&mut self, path: impl AsRef<Path>) -> eyre::Result<()> {
        let path = path.as_ref();
        self.templates.reload(path).await?;
        // FIXME: Figure out which pages need to change
        Ok(())
    }
}

/// A blog page.
///
/// This page is "owned", meaning it owns it's data. It's used for storing and caching pages around. It's cached in
/// the [`Compiler`] in order to track which pages need to recompile, and also to be passed inside the templating engine
/// [`context`] variables.
pub struct SitePage {
    /// The rendered contents of the page.
    pub contents: String,
    /// The original filename used for this page.
    pub filename: String,
    /// The filename of a Document resource without its extension (or date prefixes for a post).
    /// For example, slug for a post at URL `/2017/02/22/my-new-post.html`, would be
    /// `my-new-post`.
    pub slug: String,
    /// The URL of this page.
    pub url: String,
    /// The tags of this page. Set in the frontmatter.
    // FIXME: Allocation here.
    pub tags: Vec<String>,
    /// The title of this page. Set in the frontmatter.
    pub title: String,
    /// Whether this page is still a draft.
    pub draft: bool,
    /// Meta information about this page.
    pub meta: context::PageMeta,
}

impl<'ctx> Into<context::Page<'ctx>> for &'ctx SitePage {
    fn into(self) -> context::Page<'ctx> {
        context::Page {
            contents: &self.contents,
            filename: &self.filename,
            slug: &self.slug,
            url: &self.url,
            tags: self.tags.clone(), // FIXME: Clone
            title: &self.title,
            draft: self.draft,
            meta: self.meta,
        }
    }
}

fn get_frontmatter<'src>(
    parser: &mut pulldown_cmark::Parser<'src>,
) -> eyre::Result<frontmatter::Frontmatter> {
    let first_event = parser.next().ok_or_else(|| eyre!("missing frontmatter"))?;

    let Event::Start(Tag::MetadataBlock(MetadataBlockKind::PlusesStyle)) = first_event else {
        bail!("invalid frontmatter node")
    };

    // Collect all text content until we hit the End tag
    let mut frontmatter_content = String::new();

    loop {
        // FIXME: We can potentially get stuck in this loop? The parser should not give us a begin block without
        // an end block anyway.
        let Some(event) = parser.next() else {
            break;
        };

        match event {
            // No more frontmatter
            Event::End(TagEnd::MetadataBlock(MetadataBlockKind::PlusesStyle)) => break,
            // Otherwise keep accumulating the text in the string
            Event::Text(text) => frontmatter_content.push_str(&text),
            Event::SoftBreak | Event::HardBreak => {
                frontmatter_content.push('\n');
            }
            _ => unreachable!("inside frontmatter"),
        }
    }

    let frontmatter =
        frontmatter::from_str(&frontmatter_content).context("failed to parse frontmatter")?;
    Ok(frontmatter)
}
