//! `context` - The context passed into the templates.
//!
//! These are the possible variables you can use when doing substitution inside the liquid template
//! engine. Mostly inspired by what Jekyll and Shopify propose in their liquid template engines.

use std::collections::HashSet;

// NOTE: We only need to serialize these things
use serde::Serialize;

#[derive(Serialize)]
pub struct Site {
    /// The base URL of this website.
    pub base_url: String,
    /// All the tags from all the pages of the website.
    pub all_tags: HashSet<String>,
}

#[derive(Serialize)]
pub struct Build {
    /// Whether we are in developement mode. This is only true when we are serving the builds from
    /// the live server (provided by the `cli` crate). In this case you should include a script that
    /// loads `/reload-script.js`
    pub dev: bool,
    /// The stylesheet path.
    pub stylesheet_link: String,
}

#[derive(Serialize)]
pub struct Page<'ctx> {
    /// The rendered contents of the page.
    pub contents: &'ctx str,
    /// The original filename used for this page.
    pub filename: &'ctx str,
    /// The filename of a Document resource without its extension (or date prefixes for a post).
    /// For example, slug for a post at URL `/2017/02/22/my-new-post.html`, would be
    /// `my-new-post`.
    pub slug: &'ctx str,
    /// The URL of this page.
    pub url: &'ctx str,
    /// The tags of this page. Set in the frontmatter.
    // FIXME: Allocation here.
    pub tags: Vec<String>,
    /// The title of this page. Set in the frontmatter.
    pub title: &'ctx str,
    /// The description of this page. Set in the frontmatter.
    pub description: Option<&'ctx str>,
    /// Whether this page is still a draft.
    pub draft: bool,
    /// Meta information about this page.
    pub meta: PageMeta,
}

#[derive(Serialize, Clone, Copy)]
pub struct PageMeta {
    /// The word count of this page.
    pub word_count: usize,
    /// The time in seconds to read this page.
    pub reading_time: usize,
}

#[derive(Serialize)]
pub struct Context<'ctx> {
    pub page: Page<'ctx>,
    /// All the pages of this blog, including excluding the current one.
    pub pages: Vec<Page<'ctx>>,
    pub site: &'ctx Site,
    pub build: &'ctx Build,
}
