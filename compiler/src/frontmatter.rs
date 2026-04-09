use serde::{Deserialize, Serialize};

const fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Frontmatter {
    title: String,
    tags: Vec<String>,
    #[serde(default = "default_true")]
    draft: bool,
}

pub fn from_str(s: &str) -> Result<Frontmatter, toml::de::Error> {
    toml::from_str(s)
}
