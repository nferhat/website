//! # `highlighter` - tree-sitter based code highlighter
//!
//! This code wraps a bunch of APIs from tree-sitter, and loads parsers and queries from a configuration
//! file. It also caches them nicely if already seen before.

use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};

use eyre::{Context, Result, bail};
use libloading;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::debug;
use tree_sitter_highlight::{HighlightConfiguration, Highlighter, HtmlRenderer};

mod builder;
mod config;

pub use config::Theme;

/// The grammar cache.
///
/// Grammars are lazily loaded from the website. In other words, each websites needs to tell us which
/// grammars does it want, and also provide the highlight (and injection if they want) queries needed
/// to highlight codeblocks.
pub struct GrammarCache {
    languages_dir: PathBuf,
    cache: Mutex<HashMap<String, Arc<HighlightConfiguration>>>,
    /// The theme for the grammars. It's stored here since we also need the highlight names.
    theme: Theme,
}

impl GrammarCache {
    /// Creates a new [`GrammarCache`].
    ///
    /// It will try to load language data from the given `languages_dir`. The expected structure
    /// is as follows:
    ///
    /// - `<languages_dir>/<lang-name>/config.toml` - configuration for the language, including their names
    ///    when to inject them, the source from which their grammar should be fetched from...
    /// - `<languages_dir>/<lang-name>/*.scm` - the queries
    ///
    /// The build results will be found inside `<languages_dir>/_build/<lang-name>.so`.
    pub async fn new(languages_dir: impl AsRef<Path>) -> Result<Self> {
        let languages_dir = languages_dir.as_ref().to_path_buf();
        let theme = Theme::load_async(&languages_dir.join("theme.toml")).await?;

        Ok(GrammarCache {
            languages_dir,
            cache: Mutex::new(HashMap::new()),
            theme,
        })
    }

    /// Tries to load a grammar, by first checking if it exists in the cache.
    ///
    /// If it does not, it tries to load it from disk.
    pub async fn load(&self, name: &str) -> Result<Arc<HighlightConfiguration>> {
        // Check if the grammar is already cached
        {
            let cache = self.cache.lock().await;
            if let Some(config) = cache.get(name) {
                return Ok(Arc::clone(config));
            }
        }

        // Check if the compiled grammar exists in the build directory
        let build_dir = self.languages_dir.join("_build");
        let so_path = build_dir.join(format!("{}.so", name));

        let grammar_dir = self.languages_dir.join(name);
        if !grammar_dir.exists() {
            bail!("grammar library exists but language dir doesn't exist?")
        }

        // If the compiled grammar doesn't exist, delegate to builder
        // FIXME: Check if it needs recompile, but the user can just rm the parser dir and whatnot.
        let _config = if !so_path.exists() {
            debug!(?name, "building language grammar...");
            builder::build_grammar(name, &self.languages_dir).await
        } else {
            let config_file_path = grammar_dir.join("config.toml");
            config::Grammar::load_async(&config_file_path).await
        }?;

        // Load the HighlightConfiguration from the .so file and query files
        let mut config = Self::load_highlight_configuration(name, &so_path, &self.languages_dir)
            .await
            .context("failed to load highlight configuration")?;
        config.configure(&self.theme.highlight_names);

        let mut cache = self.cache.lock().await;
        cache.insert(name.to_string(), Arc::new(config));

        drop(cache);

        let cache = self.cache.lock().await;
        cache
            .get(name)
            .cloned()
            .ok_or_else(|| eyre::eyre!("Failed to load grammar: {}", name))
    }

    /// Loads a HighlightConfiguration from a compiled grammar library and query files.
    async fn load_highlight_configuration(
        language_name: &str,
        so_path: &Path,
        languages_dir: &Path,
    ) -> Result<HighlightConfiguration> {
        // Load the compiled grammar library
        let library = unsafe {
            libloading::Library::new(so_path).context("failed to load compiled grammar library")?
        };

        // Get the language function
        // The function name is typically tree_sitter_<language>
        let language_fn_name = format!("tree_sitter_{}", language_name);
        let language = unsafe {
            let language_fn: libloading::Symbol<extern "C" fn() -> tree_sitter::Language> = library
                .get(language_fn_name.as_bytes())
                .context("failed to find language function in compiled grammar")?;
            language_fn()
        };

        // Forget the library so it stays loaded
        // The Language pointer from the .so file needs to remain valid
        std::mem::forget(library);

        // Read the highlights.scm query file
        let highlights_path = languages_dir.join(language_name).join("highlights.scm");
        let highlights_query = fs::read_to_string(&highlights_path)
            .await
            .context("failed to read highlights.scm query file")?;

        // Try to read the injections.scm query file if it exists
        let injections_path = languages_dir.join(language_name).join("injections.scm");
        let injections_query = if injections_path.exists() {
            fs::read_to_string(&injections_path)
                .await
                .context("failed to read injections.scm query file")?
        } else {
            String::new()
        };

        // Create the HighlightConfiguration
        let config = HighlightConfiguration::new(
            language,
            language_name,
            &highlights_query,
            &injections_query,
            "", // Locals query (empty for now)
        )
        .context("failed to create highlight configuration")?;

        Ok(config)
    }

    /// Creates a stylesheet that highlights tree-sitter code generated by [`highlight`]
    pub fn stylesheet(&self) -> Result<String> {
        let mut out = String::new();

        for (name, highlight) in &self.theme.highlights {
            let css_highlight_name = name.replace('.', "-");
            write!(&mut out, ".{css_highlight_name}{{")?;
            match highlight {
                config::Style::ForegroundOnly(color) => {
                    write!(&mut out, "color:{color}")?;
                }
                config::Style::Style {
                    fg,
                    bg,
                    font_style,
                    font_weight,
                } => {
                    write!(&mut out, "color:{fg}")?;
                    if let Some(bg) = bg {
                        write!(&mut out, ";background-color:{bg}")?;
                    }
                    if let Some(font_style) = font_style {
                        write!(&mut out, ";font-style:{font_style}")?;
                    }
                    if let Some(font_weight) = font_weight {
                        write!(&mut out, ";font_weight:{font_weight}")?;
                    }
                }
            }
            write!(out, "}}")?;
        }

        Ok(out)
    }

    /// Reloads the [`Theme`] used to highlight things.
    pub async fn reload_theme(&mut self) -> Result<()> {
        self.theme = Theme::load_async(&self.languages_dir.join("theme.toml")).await?;
        Ok(())
    }

    /// Reloads a given grammar config
    pub async fn reload_grammar(&mut self, name: &str) -> Result<()> {
        let name = name.to_string(); // FIXME: Alloc
        self.cache.lock().await.remove_entry(&name);
        self.load(&name).await?;
        Ok(())
    }
}

/// Highlights the given `source_code` with the specified `grammar`.
///
/// To get the actual grammars, refer to [`GrammarCache::load`]
pub fn highlight(
    source_code: &str,
    grammar: Arc<HighlightConfiguration>,
    grammar_cache: &GrammarCache,
) -> Result<String> {
    let mut highlighter = Highlighter::new();
    let events = highlighter.highlight(&grammar, source_code.as_bytes(), None, |_name| {
        // FIXME: Injections
        None
    })?;
    let mut renderer = HtmlRenderer::new();
    let theme = &grammar_cache.theme;

    renderer.render(
        events,
        source_code.as_bytes(),
        &move |tree_sitter_highlight::Highlight(id), output| {
            output.extend(b"class='");
            let highlight_name = &theme.highlight_names[id];
            let css_highlight_name = highlight_name.replace('.', "-");
            output.extend_from_slice(css_highlight_name.as_bytes());
            output.extend(b"'");
        },
    )?;

    let highlighted_lines = renderer.lines().collect::<Vec<_>>();
    Ok(highlighted_lines.join(""))
}
