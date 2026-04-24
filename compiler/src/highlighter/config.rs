use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Serialize, Clone)]
pub struct Theme {
    pub highlights: HashMap<String, Style>,
    #[serde(skip)]
    pub(super) highlight_names: Vec<String>,
}

// Custom deserialize function to build the highlight names
impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let highlights = HashMap::<String, Style>::deserialize(deserializer)?;

        let mut highlight_names = Vec::new();
        highlight_names.reserve(highlights.len());
        for name in highlights.keys() {
            highlight_names.push(name.clone());
        }

        Ok(Self {
            highlights,
            highlight_names,
        })
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged, deny_unknown_fields)]
pub enum Style {
    ForegroundOnly(String),
    Style {
        fg: String,
        bg: Option<String>,
        font_style: Option<String>,
        font_weight: Option<usize>,
    },
}

// FIXME: Support other things than git.
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Grammar {
    /// The repository to pull the grammar from.
    pub repo: String,
    /// The git revision to use.
    pub rev: String,
    /// The subdirectory of the grammar, if any. This is needed for parsers like
    /// [tree-sitter-comment](https://github.com/stewsd/tree-sitter-comment).
    pub subdir: Option<String>,
}
