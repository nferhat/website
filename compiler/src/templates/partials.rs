//! `partials` -*- custom partial loader for our liquid renderer.
//!
//! It's meant to be used with the [`LazyCompiler`](liquid::partials::LazyCompiler). If will register all the

use std::{borrow::Cow, path::Path, sync::Arc};

#[derive(Debug)]
pub struct PartialLoader {
    template_dir: Arc<Path>,
}

impl PartialLoader {
    pub fn new(template_dir: Arc<Path>) -> Self {
        Self { template_dir }
    }
}

impl liquid::partials::PartialSource for PartialLoader {
    fn contains(&self, name: &str) -> bool {
        let path = if name.ends_with(".liquid") {
            self.template_dir.join(name)
        } else {
            self.template_dir.join(&name).with_extension("liquid")
        };

        path.exists() && path.is_file()
    }

    fn names(&self) -> Vec<&str> {
        vec![]
    }

    fn try_get<'a>(&'a self, name: &str) -> Option<Cow<'a, str>> {
        let path = if name.ends_with(".liquid") {
            self.template_dir.join(name)
        } else {
            self.template_dir.join(&name).with_extension("liquid")
        };

        let contents = std::fs::read_to_string(&path).ok()?;
        Some(Cow::Owned(contents))
    }
}
