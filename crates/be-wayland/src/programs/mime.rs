use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MimeGlobs {
    extensions: HashMap<String, BTreeSet<String>>,
    aliases: HashMap<String, String>,
}

impl MimeGlobs {
    pub fn read(data_dirs: &[PathBuf]) -> Self {
        let mut globs = Self::default();
        for dir in data_dirs.iter().rev() {
            if let Ok(text) = std::fs::read_to_string(dir.join("mime/globs2")) {
                globs.add_globs(&text);
            }
            if let Ok(text) = std::fs::read_to_string(dir.join("mime/aliases")) {
                globs.add_aliases(&text);
            }
        }
        globs
    }

    pub fn parse(globs2: &str, aliases: &str) -> Self {
        let mut globs = Self::default();
        globs.add_globs(globs2);
        globs.add_aliases(aliases);
        globs
    }

    fn add_globs(&mut self, text: &str) {
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            let mut fields = line.split(':');
            let (Some(_weight), Some(mime), Some(pattern)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let Some(extension) = pattern.strip_prefix("*.") else {
                continue;
            };
            if extension.is_empty() || extension.contains(['*', '?', '[', '.']) {
                continue;
            }
            self.extensions
                .entry(mime.to_owned())
                .or_default()
                .insert(extension.to_lowercase());
        }
    }

    fn add_aliases(&mut self, text: &str) {
        for line in text.lines() {
            if let Some((alias, canonical)) = line.split_once(' ') {
                self.aliases
                    .insert(alias.trim().to_owned(), canonical.trim().to_owned());
            }
        }
    }

    pub fn extensions(&self, mime_types: &[String]) -> Vec<String> {
        let mut found = BTreeSet::new();
        for mime in mime_types {
            let canonical = self.aliases.get(mime).unwrap_or(mime);
            for listed in [mime, canonical] {
                if let Some(extensions) = self.extensions.get(listed) {
                    found.extend(extensions.iter().cloned());
                }
            }
        }
        found.into_iter().collect()
    }
}
