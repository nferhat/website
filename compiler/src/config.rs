//! The configuration for the server

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_root_file() -> PathBuf {
    PathBuf::from("style/style.sass")
}

fn default_blog_base_path() -> String {
    String::from("blog/")
}

fn default_content_dir() -> String {
    String::from("content/")
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Config {
    /// The base URL of your website. This is where all assets are relative to.
    /// If you are linking to `/assets/image.png`, the compiler will resolve it to
    /// `https://nferhat.dev/assets/image.png`
    pub base_url: String,
    /// The content directory root.
    #[serde(default = "default_content_dir")]
    pub content_dir: String,
    /// Styling configuration.
    pub styling: StylingConfig,
    /// Configuration for the blog system.
    pub blog: BlogConfig,
}

crate::derive_config_load!(Config, "website");

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct StylingConfig {
    /// We use SASS for styling. It's not an option. You can slap some style.css inside the dist/
    /// folder if you want. This parameter controls you want SCSS or SASS (See the different on
    /// https://sass-lang.com)
    pub syntax: StyleSyntax,
    /// The root file of your styling.
    ///
    /// By default, it also adds the directory containing this file into the sass load-paths.
    #[serde(default = "default_root_file")]
    pub root_file: PathBuf,
    /// Additional directories to add to load-paths of the SASS compiler.
    ///
    /// This is needed if you have some SASS libraries shipped on npm
    /// (I mean you shouldn't use them for something this simple but I digress)
    pub load_paths: Vec<PathBuf>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleSyntax {
    Css,
    Scss,
    #[default]
    Sass,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct BlogConfig {
    /// The base path where the compiler should find blogs. This affects not only where it
    /// finds the markdown files but also where in the build folder it should output their
    /// compiled html.
    ///
    /// If you set this to `articles/` and run the compiler, all .md files will be generated
    /// inside `dist/articles/*.html`, with `dist/articles/index.html` being the "all articles"
    /// page
    #[serde(default = "default_blog_base_path")]
    pub base_path: String,
    /// Whether to include drafts into the "All blogs" page. Drafts are always accessible
    /// (using their absolute path), however they are skipped from the listing, search, tag
    /// search, etc.
    pub include_drafts: bool,
}

/// Error that can happen when reading the config.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}
