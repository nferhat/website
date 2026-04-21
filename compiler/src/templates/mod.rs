use std::{path::Path, sync::Arc};

use eyre::Result;
use tokio::fs;

pub mod context;

pub struct Templates {
    templates_dir: Arc<Path>,
    parser: liquid::Parser,
    index: liquid::Template,
}

impl Templates {
    /// Initiates a new template registry.
    ///
    /// This handles proper creation of the [`liquid::Parser`] used to create and manage the templates,
    /// and loads the index template.
    pub async fn new(templates_dir: impl AsRef<Path>) -> Result<Self> {
        let dir = templates_dir.as_ref();

        // We create the parser that's gonna create all the liquid::Templates
        let parser = liquid::ParserBuilder::new().stdlib().build()?;
        let index = Self::load_template_internal(&dir.join("index.liquid"), &parser).await?;

        let templates_dir = dir.to_path_buf().into_boxed_path();

        Ok(Self {
            templates_dir: templates_dir.into(),
            parser,
            index,
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

        // Update the index template if reloading that file
        if full_path.ends_with("index.liquid") {
            self.index = template;
        }

        // FIXME: Other templates

        Ok(())
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
