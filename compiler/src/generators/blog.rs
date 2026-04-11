use std::fmt::{self, Write};

use crate::frontmatter::Frontmatter;

static BLOG_PAGE_BASE: &'static str = include_str!("../../templates/blog-page.html");
static MATHJAX_SCRIPT_URL: &'static str = "https://cdn.jsdelivr.net/npm/mathjax@4/tex-mml-chtml.js";

/// Generates a single blog page using the [`BLOG_PAGE_BASE`].
/// If you need changes, it should be to that base HTML file.
///
/// This function is basically a fancy find-and-replace that creates a structured HTML document.
pub fn page(
    frontmatter: &Frontmatter,
    content: &str,
    stylesheet_path: Option<&str>,
    include_math_script: bool,
) -> crate::Result<String> {
    let out = BLOG_PAGE_BASE;

    let mut opengraph_metadata = String::with_capacity(512);
    write!(
        &mut opengraph_metadata,
        "<title>{}</title>",
        frontmatter.title
    )?;
    write_opengraph_metadata(
        &frontmatter.title,
        None, // FIX: Add in frontmatter
        Some("article"),
        None, // FIX: Compute URL
        &mut opengraph_metadata,
    )?;
    let out = out.replace("|OPENGRAPH_METADATA|", &opengraph_metadata);

    let stylesheet_path = stylesheet_path.unwrap_or("/style.css");
    let out = out.replace("|STYLESHEET|", &stylesheet_path);

    let mut additional_head_nodes = String::new();

    if include_math_script {
        write!(
            &mut additional_head_nodes,
            r#"
            <script> MathJax = {{ options: {{ enableMenu: false }} }}; </script>
            <script defer id="mathjax" src="{MATHJAX_SCRIPT_URL}"></script>"#
        )?;
    }
    let out = out.replace("|HEAD_NODES|", &additional_head_nodes);

    Ok(out.replace("|CONTENT|", content))
}

fn write_opengraph_metadata<'a>(
    title: &'a str,
    description: Option<&'a str>,
    content_type: Option<&'a str>,
    url: Option<&'a str>,
    out: &mut impl fmt::Write,
) -> fmt::Result {
    write!(out, r#"<meta property="og:title" content="{title}" />"#)?;

    // All these properties are optional. If I am not mistaken these are not strictly required
    // for platforms to create your embed. Title is enough to make discord happy.
    if let Some(desc) = description {
        write!(
            out,
            r#"<meta property="og:description" content="{desc}" />"#
        )?;
    }
    if let Some(ct) = content_type {
        write!(out, r#"<meta property="og:type" content="{ct}" />"#)?;
    }
    if let Some(url) = url {
        write!(out, r#"<meta property="og:url" content="{url}" />"#)?;
    }

    Ok(())
}
