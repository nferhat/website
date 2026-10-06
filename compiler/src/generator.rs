//! HTML renderer that takes an iterator of [`jotdown`] events as input.
//!
//! Most of the heavy lifting is done by [`jotdown::html::Renderer`]. This module only intercepts the events
//! that need special treatment for my website and renders them by hand:
//!
//! - Code blocks, which get highlighted with tree-sitter and wrapped in a `codeblock` container.
//! - Images, which get loaded through the asset registry and wrapped in an `image-container`.
//! - Inline code, which gets an `inline-code` class.
//!
//! Intercepted blocks are handed back to the renderer as raw HTML events, so it can keep track of
//! everything else (footnotes, list tightness, indentation, ...) on its own.

use std::fmt::{self, Write};
use std::path::Path;

use jotdown::{
    AttributeKind, Attributes, Container, Event, LinkType, Render, SpanLinkType, html::Renderer,
};

use crate::assets::AssetRegistry;
use crate::highlighter::{self, GrammarCache};

/// Writes `s` into `out`, escaping the characters that are special in HTML text and attribute values.
fn escape_html(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

/// Feeds a chunk of raw HTML to the renderer, as a raw block.
fn push_raw_block<'a, W: Write>(
    renderer: &mut Renderer<'a>,
    html: String,
    out: &mut W,
) -> fmt::Result {
    let container = Container::RawBlock {
        format: "html".into(),
    };
    renderer.push_event(Event::Start(container.clone(), Attributes::new()), &mut *out)?;
    renderer.push_event(Event::Str(html.into()), &mut *out)?;
    renderer.push_event(Event::End(container.clone()), &mut *out)
}

/// Feeds a chunk of raw HTML to the renderer, as raw inline content.
fn push_raw_inline<'a, W: Write>(
    renderer: &mut Renderer<'a>,
    html: String,
    out: &mut W,
) -> fmt::Result {
    let container = || Container::RawInline {
        format: "html".into(),
    };
    renderer.push_event(Event::Start(container(), Attributes::new()), &mut *out)?;
    renderer.push_event(Event::Str(html.into()), &mut *out)?;
    renderer.push_event(Event::End(container()), &mut *out)
}

/// Renders a code block, consuming events up to (and including) the end of the code block.
async fn render_code_block<'a, I>(
    iter: &mut I,
    language: &str,
    grammar_cache: &GrammarCache,
) -> String
where
    I: Iterator<Item = Event<'a>>,
{
    let mut code = String::new();
    for event in iter {
        match event {
            Event::Str(text) => code.push_str(&text),
            Event::End(Container::CodeBlock { .. }) => break,
            _ => (),
        }
    }

    let mut out = String::with_capacity(code.len() * 2);
    out.push_str("<div class=codeblock>");
    if language.is_empty() {
        out.push_str("<pre><code>");
    } else {
        out.push_str("<p class=language-name>");
        escape_html(&mut out, language);
        out.push_str("</p><pre><code class=\"");
        escape_html(&mut out, language);
        out.push_str("\">");
    }

    // In order to do tree-sitter highlighting, we first have to load the language.
    let mut highlighted = None;
    if !language.is_empty() {
        if let Ok(grammar) = grammar_cache.load(language).await {
            match highlighter::highlight(&code, grammar, grammar_cache) {
                Ok(rendered) => highlighted = Some(rendered),
                Err(err) => warn!(?err, "failed to highlight codeblock"),
            }
        }
    }

    match highlighted {
        Some(rendered) => out.push_str(&rendered),
        None => escape_html(&mut out, &code),
    }
    out.push_str("</code></pre></div>\n");
    out
}

/// Renders an image, consuming events up to (and including) the end of the image.
async fn render_image<'a, I>(
    iter: &mut I,
    src: &str,
    attrs: &Attributes<'a>,
    source_path: &Path,
    asset_registry: &AssetRegistry,
) -> String
where
    I: Iterator<Item = Event<'a>>,
{
    // The alt text is the plain text content of the image description.
    let mut alt = String::new();
    let mut nest = 0;
    for event in iter {
        match event {
            Event::Start(..) => nest += 1,
            Event::End(..) if nest == 0 => break,
            Event::End(..) => nest -= 1,
            Event::Str(text) => alt.push_str(&text),
            Event::Softbreak | Event::Hardbreak => alt.push(' '),
            _ => (),
        }
    }

    // HACK: If image is from the web don't touch it.
    // I should find a better way to determine if an image should be from us (IE asset)
    // or not. Whatever.
    let src = if src.starts_with("http") {
        src.to_string()
    } else {
        // Try to load from asset registry or just bail out and use what the user asks
        // for. I don't know how to make it better right now.
        asset_registry
            .load(src, source_path)
            .await
            .unwrap_or_else(|_| src.to_string())
    };

    // Djot images have no title syntax, instead it is given as an attribute: `![alt](src){title="..."}`
    let title = attrs
        .get_value("title")
        .map(|title| title.to_string())
        .filter(|title| !title.is_empty());

    let mut out = String::new();
    out.push_str("<div class=image-container><img class=clickable-image src=\"");
    escape_html(&mut out, &src);
    out.push_str("\" alt=\"");
    escape_html(&mut out, &alt);
    if let Some(title) = &title {
        out.push_str("\" title=\"");
        escape_html(&mut out, title);
    }
    out.push_str("\" />");
    if let Some(title) = &title {
        out.push_str("<span class=image-title>");
        escape_html(&mut out, title);
        out.push_str("</span>");
    }
    out.push_str("</div>");
    out
}

pub async fn write_html_fmt<'a, I, W>(
    mut writer: W,
    mut iter: I,
    source_path: &Path,
    grammar_cache: &GrammarCache,
    asset_registry: &AssetRegistry,
) -> fmt::Result
where
    I: Iterator<Item = Event<'a>>,
    W: Write,
{
    let mut renderer = Renderer::default();

    while let Some(event) = iter.next() {
        match event {
            Event::Start(Container::CodeBlock { language }, _) => {
                let html = render_code_block(&mut iter, &language, grammar_cache).await;
                push_raw_block(&mut renderer, html, &mut writer)?;
            }
            Event::Start(Container::Image(src, _), attrs) => {
                let html =
                    render_image(&mut iter, &src, &attrs, source_path, asset_registry).await;
                push_raw_inline(&mut renderer, html, &mut writer)?;
            }
            Event::Start(Container::Verbatim, mut attrs) => {
                attrs.push((AttributeKind::Class, "inline-code".into()));
                renderer.push_event(Event::Start(Container::Verbatim, attrs), &mut writer)?;
            }
            Event::Start(Container::Link(label, LinkType::Span(SpanLinkType::Unresolved)), attrs) => {
                warn!(%label, "Found broken link");
                renderer.push_event(
                    Event::Start(
                        Container::Link(label, LinkType::Span(SpanLinkType::Unresolved)),
                        attrs,
                    ),
                    &mut writer,
                )?;
            }
            event => renderer.push_event(event, &mut writer)?,
        }
    }

    Ok(())
}
