use std::{
    collections::{HashMap, hash_map::Entry},
    path::Path,
    sync::Arc,
};

use eyre::{ContextCompat, Result, bail};
use tokio::fs;

use crate::utils;

pub mod context;
mod partials;

pub struct Templates {
    templates_dir: Arc<Path>,
    parser: liquid::Parser,

    index: liquid::Template,
    /// The templates that we load.
    named_templates: HashMap<String, liquid::Template>,
}

impl Templates {
    /// Initiates a new template registry.
    ///
    /// This handles proper creation of the [`liquid::Parser`] used to create and manage the templates,
    /// and loads the index template.
    pub async fn new(templates_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = templates_dir.as_ref();
        let templates_dir = dir.to_path_buf().into_boxed_path();
        let templates_dir = Arc::from(templates_dir);

        let partials_source = partials::PartialLoader::new(Arc::clone(&templates_dir));
        let partials = liquid::partials::LazyCompiler::new(partials_source);

        // We create the parser that's gonna create all the liquid::Templates
        let parser = liquid::ParserBuilder::new()
            .stdlib()
            .partials(partials)
            .build()?;
        let index = Self::load_template_internal(&dir.join("index.liquid"), &parser).await?;

        Ok(Self {
            templates_dir: templates_dir.into(),
            parser,
            index,
            named_templates: HashMap::new(),
        })
    }

    /// Reloads a given template.
    pub async fn reload(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        // FIXME: Should absolute paths to templates resolve?
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.templates_dir.join(path)
        };

        // Load the new template
        let template = Self::load_template_internal(&full_path, &self.parser).await?;

        let file_name = full_path
            .file_name()
            .context("template without filename")?
            .to_string_lossy();

        match file_name.trim() {
            "index.liquid" => self.index = template,
            _ => {
                let path = utils::strip_leading_segment(&full_path, &self.templates_dir);
                let key = path.to_string_lossy().to_string();
                self.named_templates.insert(key, template);
            }
        }

        // FIXME: Other templates

        Ok(())
    }

    /// Tries to get a template from the cached `named_templates`. If it cannot find it, it will try
    /// loading them from disk.
    ///
    /// Note that asking for `blog-page.liquid` or `blog-page` does the same thing.
    pub async fn get_template(&mut self, name: &str) -> Result<&liquid::Template> {
        let name = if name.ends_with(".liquid") {
            name.to_string()
        } else {
            format!("{name}.liquid")
        };

        match self.named_templates.entry(name.clone()) {
            Entry::Occupied(occupied_entry) => {
                trace!(?name, "Loading cached template");
                Ok(&*occupied_entry.into_mut())
            }
            Entry::Vacant(vacant_entry) => {
                let path = self.templates_dir.join(&name);
                if !path.exists() || !path.is_file() {
                    bail!("unable to find template '{name}'");
                }

                let template = Self::load_template_internal(&path, &self.parser).await?;
                Ok(&*vacant_entry.insert(template))
            }
        }
    }

    pub const fn index_template(&self) -> &liquid::Template {
        &self.index
    }

    async fn load_template_internal(
        path: &Path,
        parser: &liquid::Parser,
    ) -> Result<liquid::Template> {
        let template_text = fs::read_to_string(&path).await?;
        let template = parser.parse(&template_text)?;
        Ok(template)
    }
}
