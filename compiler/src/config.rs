//! The configuration for the server

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

fn default_root_file() -> PathBuf {
    PathBuf::from("style/style.sass")
}

fn default_content_dir() -> String {
    String::from("content/")
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
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
}

crate::derive_config_load!(Config, "website");

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
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
    /// Fonts to bundle with the styling. These will be minified and subsetted in order to
    /// have the smallest footprint possible.
    pub bundle_fonts: Vec<PathBuf>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StyleSyntax {
    Css,
    Scss,
    #[default]
    Sass,
}

/// Error that can happen when reading the config.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
}
