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
    path::{self, Path},
    sync::Arc,
};

pub use config::{BlogConfig, Config, Error as ConfigError, StylingConfig};
pub use generator::Generator;
use markdown::mdast;
pub use style::compile_to_stylesheet;
use tokio::{fs, io};

use templates::{Templates, context};

/// A result type that can be generated when compiling a site.
type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Formatting error: {0}")]
    Format(#[from] std::fmt::Error),
    #[error("Config error: {0}")]
    Config(#[from] config::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Sass error: {0}")]
    Sass(#[from] Box<grass::Error>),
    #[error("Markdown error: {0}")]
    Markdown(String),
    #[error("Missing frontmatter")]
    MissingFrontmatter,
    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(#[from] toml::de::Error),
    #[error("Liquid error")]
    Liquid(#[from] liquid::Error),
}

/// The main compiler.
pub struct Compiler {
    /// The root of the website. All paths inside the configuration and such will be
    /// relative to this path.
    root: Arc<Path>,
    // Pre-cache some paths here because we are going to use them a lot.
    #[allow(unused)]
    build_path: Arc<Path>,
    style_output_path: Arc<Path>,

    /// The templates for this website.
    templates: Templates,

    /// Website configuration
    config: Arc<Config>,

    /// The cached [`Site`](context::Site) context passed into the templates.
    site_ctx: context::Site,
    build_ctx: context::Build,
}

impl Compiler {
    pub fn new(root: impl Into<Arc<Path>>, config: &Arc<Config>) -> Self {
        let root = root.into();
        let root = path::absolute(root).unwrap();
        let root = Arc::<Path>::from(root.into_boxed_path());

        let build_path = root.join("dist").into_boxed_path();
        let style_output_path = build_path.join("style.css").into_boxed_path();
        let templates_dir = root.join("templates");
        let templates = futures::executor::block_on(Templates::new(&templates_dir)).unwrap();

        // Create and cache contexts here.
        let config = Arc::clone(config);
        let site_ctx = context::Site {
            base_url: config.base_url.clone(),
        };
        let build_ctx = context::Build { dev: false };

        Self {
            root,
            build_path: Arc::from(build_path),
            style_output_path: Arc::from(style_output_path),
            templates,
            config,
            site_ctx,
            build_ctx,
        }
    }

    pub fn set_dev_mode(&mut self, dev_mode: bool) {
        self.build_ctx.dev = dev_mode;
    }

    /// Do a full-pass compile.
    ///
    /// This goes over all the pages of the website and generates everything. This also cleans up
    /// any cached page from incremental compilation.
    pub async fn compile_all(&mut self) -> Result<()> {
        self.recompile_stylesheets().await?;

        let root = Arc::clone(&self.root);
        self.recompile_pages(&root).await?;

        Ok(())
    }

    async fn recompile_pages(&mut self, dir: impl AsRef<Path>) -> Result<()> {
        let mut stack = Vec::with_capacity(10);
        stack.push(dir.as_ref().to_owned());

        while let Some(dir) = stack.pop() {
            let mut read_dir = fs::read_dir(dir).await?;
            'entries: while let Some(entry) = read_dir.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    // push it onto the stack and continue with the next entry
                    stack.push(path);
                    continue 'entries;
                } else {
                    let Some(ext) = path.extension() else {
                        continue 'entries;
                    };

                    if ext != "md" && ext != "markdown" && ext != "mdown" {
                        continue 'entries; // nope not interested
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
    pub async fn recompile_stylesheets(&mut self) -> Result<()> {
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
            Ok(res) => res.map_err(Error::Sass),
            Err(_) => Err(Error::Io(io::Error::new(
                io::ErrorKind::Other,
                "background task failed",
            ))),
        }?;

        fs::write(&self.style_output_path, style_contents).await?;

        Ok(())
    }

    /// Does a partial recompilation of the given `input_path`.
    ///
    /// It figures out the needed other pages that need to recompile in extra to this one,
    /// for example if you ask to recompile a blog page, the blogs index page will also update.
    pub async fn recompile_page(&mut self, input_path: impl AsRef<Path>) -> Result<()> {
        let input_path = input_path.as_ref();

        // FIXME: Figure out depends of this page.
        // This is easier said than done, however.
        let raw_content = fs::read_to_string(input_path).await?;
        let mut contents = markdown::to_mdast(&raw_content, &parse_options())
            .map_err(|msg| Error::Markdown(msg.to_string()))?;

        // Get the frontmatter out.
        let children = contents.children_mut().ok_or(Error::MissingFrontmatter)?;
        if children.len() < 1 {
            return Err(Error::MissingFrontmatter);
        }
        let frontmatter_node = children.remove(0);
        let mdast::Node::Toml(mdast::Toml {
            value: frontmatter_str,
            ..
        }) = frontmatter_node
        else {
            return Err(Error::MissingFrontmatter);
        };

        let frontmatter = frontmatter::from_str(&frontmatter_str)?;
        let path_relative = input_path.strip_prefix(&self.root).expect("root path");
        let output_path = self.build_path.join(path_relative).with_extension("html");
        // We can calculate the resulting URL from the output path.
        let url = path_relative.with_extension("html");
        let url = url.to_string_lossy();

        let filename = input_path.file_name().unwrap().to_string_lossy();
        let slug = {
            let slug = input_path.file_stem().unwrap();
            slug.to_string_lossy()
        };

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
            },
            site: &self.site_ctx,
            build: &self.build_ctx,
        };

        let context = liquid::to_object(&context)?;
        let content = template.render(&context)?;

        fs::write(&output_path, content).await?;

        Ok(())
    }

    /// Reloads a given template path.
    pub async fn reload_template(&mut self, path: impl AsRef<Path>) -> Result<()> {
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
