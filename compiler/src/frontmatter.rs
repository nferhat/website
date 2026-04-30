use liquid::model::DateTime;
use serde::{Deserialize, Serialize, de};

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
#[serde(rename_all = "kebab-case")]
pub struct Frontmatter {
    /// The title of this page.
    pub title: String,
    /// The release date of this article/page
    #[serde(deserialize_with = "deserialize_date", default)]
    pub release_date: Option<DateTime>,
    /// The description of this page.
    pub description: Option<String>,
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
    /// The template used to rendering this page.
    pub template: Option<String>,
}

pub fn from_str(s: &str) -> Result<Frontmatter, toml::de::Error> {
    toml::from_str(s)
}

fn deserialize_date<'de, D: de::Deserializer<'de>>(
    de: D,
) -> Result<Option<DateTime>, <D as de::Deserializer<'de>>::Error> {
    let string = Option::<String>::deserialize(de)?;
    let string = match string {
        Some(s) => s,
        None => return Ok(None),
    };

    let mut parts = string.split('-');

    let year = parts
        .next()
        .ok_or_else(|| <D::Error as de::Error>::missing_field("year"))?;
    let year = year
        .parse()
        .ok()
        .ok_or_else(|| <D::Error as de::Error>::custom("year should be a number"))?;

    let month = parts
        .next()
        .ok_or_else(|| <D::Error as de::Error>::missing_field("month"))?;
    let month = month
        .parse()
        .ok()
        .ok_or_else(|| <D::Error as de::Error>::custom("month should be a number"))?;

    let day = parts
        .next()
        .ok_or_else(|| <D::Error as de::Error>::missing_field("day"))?;
    let day = day
        .parse()
        .ok()
        .ok_or_else(|| <D::Error as de::Error>::custom("day should be a number"))?;

    Ok(Some(DateTime::from_ymd(year, month, day)))
}
