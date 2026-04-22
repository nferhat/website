use std::{
    env,
    path::{Component, Path, PathBuf},
};

use markdown::mdast::Node;

/// Counts all the words inside of a given node. You should pass in the root node
/// here to get all the words inside a document.
#[allow(unused)]
pub fn count_words(node: &Node) -> usize {
    match node {
        Node::Text(text) => count_words_in_text(&text.value),
        Node::InlineCode(code) => count_words_in_text(&code.value),
        Node::Root(n) => n.children.iter().map(count_words).sum(),
        Node::Paragraph(n) => n.children.iter().map(count_words).sum(),
        Node::Heading(n) => n.children.iter().map(count_words).sum(),
        Node::Strong(n) => n.children.iter().map(count_words).sum(),
        Node::Emphasis(n) => n.children.iter().map(count_words).sum(),
        Node::Delete(n) => n.children.iter().map(count_words).sum(),
        Node::Link(n) => n.children.iter().map(count_words).sum(),
        Node::LinkReference(n) => n.children.iter().map(count_words).sum(),
        Node::Blockquote(n) => n.children.iter().map(count_words).sum(),
        Node::List(n) => n.children.iter().map(count_words).sum(),
        Node::ListItem(n) => n.children.iter().map(count_words).sum(),
        Node::Table(n) => n.children.iter().map(count_words).sum(),
        Node::TableRow(n) => n.children.iter().map(count_words).sum(),
        Node::TableCell(n) => n.children.iter().map(count_words).sum(),

        // not including code blocks? I dont know if this is really correct but eh
        _ => 0,
    }
}

#[allow(unused)]
fn count_words_in_text(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Converts a path into a `./`-prefixed path relative to the current working directory.
///
/// Behavior:
/// - If `path` is inside the current working directory (CWD), returns a relative path
///   prefixed with `./` (e.g., `/cwd/foo/bar` → `./foo/bar`).
/// - If `path` is already relative, it is normalized to start with `./`.
/// - If `path` is exactly the CWD, returns `"."`.
/// - If `path` cannot be made relative (e.g., different filesystem root or drive),
///   the original path is returned unchanged.
///
/// Edge cases handled:
/// - Symlinks are resolved via `canonicalize` when needed.
/// - Non-existent paths or canonicalization failures fall back gracefully.
/// - Works across platforms (uses `.\` on Windows automatically via `PathBuf`).
///
/// This helps me cleanly and easily strip prefixes of paths.
pub fn to_dot_relative(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let cwd = match env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return path.to_path_buf(),
    };

    let relative = match path.strip_prefix(&cwd) {
        Ok(rel) => rel.to_path_buf(),
        Err(_) => match (path.canonicalize(), cwd.canonicalize()) {
            (Ok(abs_path), Ok(abs_cwd)) => match abs_path.strip_prefix(&abs_cwd) {
                Ok(rel) => rel.to_path_buf(),
                Err(_) => return path.to_path_buf(),
            },
            _ => return path.to_path_buf(),
        },
    };

    if relative.components().next().is_none() {
        PathBuf::from(".")
    } else {
        let mut result = PathBuf::from(".");
        result.push(relative);
        result
    }
}

/// Strips a leading path segment (like "content") from a path,
/// ignoring an optional leading `./`.
///
/// ### Examples:
/// Trying to strip "content"
///
/// - "./content/a" -> "a"
/// - "content/a"   -> "a"
/// - "./a"         -> "./a" (unchanged)
/// - "other/a"     -> "other/a" (unchanged)
pub fn strip_leading_segment(path: &Path, prefix: impl AsRef<Path>) -> PathBuf {
    // Normalize away leading "./"
    let mut components = path.components().peekable();

    if let Some(Component::CurDir) = components.peek() {
        components.next();
    }

    let normalized: PathBuf = components.collect();

    // Try to strip the prefix
    match normalized.strip_prefix(prefix) {
        Ok(stripped) => {
            if stripped.components().next().is_none() {
                PathBuf::from(".")
            } else {
                stripped.to_path_buf()
            }
        }
        Err(_) => path.to_path_buf(), // return original (not normalized) if no match
    }
}
