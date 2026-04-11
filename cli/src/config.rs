//! The configuration for the server

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

fn default_root_file() -> PathBuf {
    PathBuf::from("style/style.sass")
}

fn default_blog_base_path() -> PathBuf {
    PathBuf::from("blog/")
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Config {
    /// The base URL of your website. This is where all assets are relative to.
    /// If you are linking to `/assets/image.png`, the compiler will resolve it to
    /// `https://nferhat.dev/assets/image.png`
    pub base_url: String,
    /// Styling configuration.
    pub styling: Styling,
    /// Configuration for the blog system.
    pub blog: Blog,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Styling {
    /// We use SASS for styling. It's not an option. You can slap some style.css inside the dist/
    /// folder if you want. This parameter controls you want SCSS or SASS (See the different on
    /// https://sass-lang.com)
    #[serde(default = "default_true")]
    pub use_sass: bool,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Blog {
    /// The base path where the compiler should find blogs. This affects not only where it
    /// finds the markdown files but also where in the build folder it should output their
    /// compiled html.
    ///
    /// If you set this to `articles/` and run the compiler, all .md files will be generated
    /// inside `dist/articles/*.html`, with `dist/articles/index.html` being the "all articles"
    /// page
    #[serde(default = "default_blog_base_path")]
    pub base_path: PathBuf,
    /// Whether to include drafts into the "All blogs" page. Drafts are always accessible
    /// (using their absolute path), however they are skipped from the listing, search, tag
    /// search, etc.
    pub include_drafts: bool,
}

pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Config> {
    let path = path.as_ref();
    debug!(?path, "Loading website configuration");
    let contents = std::fs::read_to_string(path)?;
    let config = toml::from_str(&contents)?;
    Ok(config)
}
