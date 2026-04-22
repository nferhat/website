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
mod generators;
mod style;
mod templates;
mod utils;

use std::{
    path::{self, Path, PathBuf},
    sync::Arc,
};

use eyre::{Context, bail, eyre};
use markdown::mdast;
use tokio::{fs, io};
use utils::to_dot_relative;

pub use config::{BlogConfig, Config, Error as ConfigError, StylingConfig};
pub use generator::Generator;
pub use style::compile_to_stylesheet;

use templates::{Templates, context};

use crate::utils::strip_leading_segment;

/// The main compiler.
pub struct Compiler {
    /// The root of the website. All paths inside the configuration and such will be
    /// relative to this path.
    root: PathBuf,
    // Pre-cache some paths here because we are going to use them a lot.
    #[allow(unused)]
    build_path: PathBuf,
    style_output_path: PathBuf,

    /// The templates for this website.
    templates: Templates,

    /// Website configuration
    config: Arc<Config>,

    /// The cached [`Site`](context::Site) context passed into the templates.
    site_ctx: context::Site,
    build_ctx: context::Build,
}

impl Compiler {
    pub fn new(root: impl AsRef<Path>, config: &Arc<Config>) -> eyre::Result<Self> {
        let root = path::absolute(root)?;

        let build_path = root.join("dist");
        let style_output_path = build_path.join("style.css");

        let templates_dir = root.join("templates");
        let templates = futures::executor::block_on(Templates::new(&templates_dir))?;

        // Create and cache contexts here.
        let config = Arc::clone(config);
        let site_ctx = context::Site {
            base_url: config.base_url.clone(),
        };
        let build_ctx = context::Build { dev: false };

        Ok(Self {
            root,
            build_path,
            style_output_path,
            templates,
            config,
            site_ctx,
            build_ctx,
        })
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

        let raw_content = fs::read_to_string(&input_path).await?;
        let mut contents = markdown::to_mdast(&raw_content, &parse_options())
            .map_err(|msg| eyre!("failed to parse markdown: {msg}"))?;

        // Get the frontmatter out.
        let children = contents
            .children_mut()
            .ok_or_else(|| eyre!("missing frontmatter"))?;
        if children.len() < 1 {
            bail!("missing frontmatter")
        }
        let frontmatter_node = children.remove(0);
        let mdast::Node::Toml(mdast::Toml {
            value: frontmatter_str,
            ..
        }) = frontmatter_node
        else {
            bail!("invalid frontmatter format, we use TOML for frontmatter")
        };

        let frontmatter = frontmatter::from_str(&frontmatter_str)?;
        let content_path = strip_leading_segment(&input_path, "content");
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
        let word_count = utils::count_words(&contents);
        let reading_time = ((word_count as f64 / WPM) * 60.0).round() as usize;

        let body = Generator::new(contents).to_html()?;

        // FIXME: figure out which templates to use
        let template = self.templates.index_template();

        let context = context::Context {
            page: context::Page {
                contents: &body,
                filename: &*filename,
                title: &frontmatter.title,
                slug: &*slug,
                url: &*url,
                tags: frontmatter.tags.clone(),
                meta: context::PageMeta {
                    reading_time,
                    word_count,
                },
            },
            site: &self.site_ctx,
            build: &self.build_ctx,
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
            math_text: true,
            ..markdown::Constructs::gfm()
        },
        ..Default::default()
    }
}
