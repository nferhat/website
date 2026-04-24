use std::{
    env,
    path::{Component, Path, PathBuf},
};

use rand::{RngExt, distr::Alphanumeric};

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

/// Generates a short random string of given `len`
pub fn random_string(len: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
