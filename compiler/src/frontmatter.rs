use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

/// Frontmatter of a page.
///
/// The frontmatter is some metadata that is specified using markdown at the top
/// of each page file. It looks something like the following:
///
/// ```md
/// +++
/// title = "Hello world"
/// tags = ["a", "b"]
/// +++
/// ```
///
/// The expected syntax is [TOML](https://toml.io), as per some random markdown extension
/// supported by the [`markdown`] crate.
///
/// All pages should have a frontmatter, since it's used to generate the "All Pages" page.
/// (funny wording, I know)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Frontmatter {
    /// The title of this page.
    pub title: String,
    /// The tags of this page.
    #[serde(default = "Vec::new")]
    pub tags: Vec<String>,
    /// Whether this page is a draft.
    ///
    /// If the page is a draft, it won't be mentionned in the list of all pages, and can only be accessed using
    /// it's link (if you know it, of course). This is essentially a unlisted option.
    #[serde(default = "default_true")]
    pub draft: bool,
    /// Whether to include the little metadata line below the page title.
    #[serde(default)]
    pub metadata: bool,
}

pub fn from_str(s: &str) -> Result<Frontmatter, toml::de::Error> {
    toml::from_str(s)
}
