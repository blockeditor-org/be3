use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};

use block_plugin_api::BlockTypeDescriptor;
use block_plugin_manifest::PluginManifest;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
mod bundle;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use bundle::read;
#[cfg(target_os = "android")]
pub(crate) use bundle::use_assets;
#[cfg(not(target_arch = "wasm32"))]
use bundle::{fetch, scan};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::load;
#[cfg(target_arch = "wasm32")]
use web::{fetch, scan};

const INDEX: &str = "plugins.json";

pub(crate) struct Plugins {
    manifests: Vec<Arc<PluginManifest>>,
    types: HashMap<Uuid, BlockTypeDescriptor>,
    errors: Vec<String>,
}

impl Default for Plugins {
    fn default() -> Self {
        Self {
            manifests: Vec::new(),
            types: block_types(&[]),
            errors: Vec::new(),
        }
    }
}

pub(crate) fn block_types(manifests: &[Arc<PluginManifest>]) -> HashMap<Uuid, BlockTypeDescriptor> {
    crate::be::host_block_types()
        .into_iter()
        .chain(
            manifests
                .iter()
                .flat_map(|manifest| &manifest.block_types)
                .map(|block_type| block_type.descriptor.clone()),
        )
        .map(|descriptor| (Uuid::from_bytes(descriptor.block_type), descriptor))
        .collect()
}

impl Plugins {
    pub(crate) fn manifests(&self) -> &[Arc<PluginManifest>] {
        &self.manifests
    }

    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
    }

    fn add(&mut self, source: &str, document: &str) {
        let manifest = match block_plugin_manifest::manifest_from_json(document) {
            Ok(manifest) => manifest,
            Err(error) => {
                self.errors.push(format!("{source}: {error}"));
                return;
            }
        };
        if let Some(block_type) = manifest
            .block_types
            .iter()
            .map(|block_type| Uuid::from_bytes(block_type.content_type()))
            .find(|block_type| self.types.contains_key(block_type))
        {
            self.errors.push(format!(
                "{source}: {block_type} is a block type the app or a plugin found first already provides"
            ));
            return;
        }
        if let Some(editor) = manifest.editors.iter().find(|editor| {
            self.manifests
                .iter()
                .flat_map(|existing| &existing.editors)
                .any(|existing| existing.id == editor.id)
        }) {
            self.errors.push(format!(
                "{source}: {} is already the id of an editor that was found first",
                Uuid::from_bytes(editor.id)
            ));
            return;
        }
        if let Some(existing) = self
            .manifests
            .iter()
            .find(|existing| existing.identity.id == manifest.identity.id)
        {
            self.errors.push(format!(
                "{source}: {} is already the id of a plugin that was found first",
                existing.identity.id
            ));
            return;
        }
        for block_type in &manifest.block_types {
            self.types.insert(
                Uuid::from_bytes(block_type.content_type()),
                block_type.descriptor.clone(),
            );
        }
        self.manifests.push(Arc::new(manifest));
    }

    fn settle(&mut self) {
        loop {
            let Some(index) = self.manifests.iter().position(|manifest| {
                Self::unprovided(manifest).any(|block_type| !self.types.contains_key(&block_type))
            }) else {
                return;
            };
            let manifest = self.manifests.remove(index);
            let missing = Self::unprovided(&manifest)
                .find(|block_type| !self.types.contains_key(block_type))
                .unwrap_or_default();
            self.errors.push(format!(
                "{}: {missing} is not a block type the app or any plugin provides",
                manifest.identity.id
            ));
            self.types = block_types(&self.manifests);
        }
    }

    fn unprovided(manifest: &PluginManifest) -> impl Iterator<Item = Uuid> + '_ {
        manifest
            .editors
            .iter()
            .flat_map(|editor| {
                editor
                    .edits
                    .block_type()
                    .into_iter()
                    .chain(editor.templates.iter().map(|template| template.block_type))
            })
            .map(Uuid::from_bytes)
    }

    fn error(&mut self, source: &str, error: impl std::fmt::Display) {
        self.errors.push(format!("{source}: {error}"));
    }
}

static PLUGINS: RwLock<Option<Arc<Plugins>>> = RwLock::new(None);

pub(crate) fn plugins() -> Arc<Plugins> {
    if let Some(plugins) = PLUGINS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return Arc::clone(plugins);
    }
    let mut held = PLUGINS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(held.get_or_insert_with(|| Arc::new(scan())))
}

#[cfg(target_arch = "wasm32")]
fn install(plugins: Plugins) {
    *PLUGINS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(plugins));
}

pub(crate) fn block_type(content_type: Uuid) -> Option<BlockTypeDescriptor> {
    plugins().types.get(&content_type).cloned()
}

pub(crate) fn exported_whole(content_type: Uuid) -> bool {
    plugins()
        .manifests
        .iter()
        .flat_map(|manifest| &manifest.block_types)
        .any(|block_type| {
            block_type.exported_whole && Uuid::from_bytes(block_type.content_type()) == content_type
        })
}

async fn discover<F>(read: impl Fn(String) -> F) -> Plugins
where
    F: Future<Output = Result<Vec<u8>, String>>,
{
    let (id, index) = futures_util::join!(read(build_id::FILE.to_owned()), read(INDEX.to_owned()));
    if let Ok(id) = id {
        let _ = build_id::install_metadata(&String::from_utf8_lossy(&id));
    }
    let mut plugins = Plugins::default();
    let manifests = index.and_then(|document| {
        serde_json::from_slice::<Vec<String>>(&document).map_err(|error| error.to_string())
    });
    match manifests {
        Ok(manifests) => {
            let documents = futures_util::future::join_all(
                manifests.iter().map(|manifest| read(manifest.clone())),
            )
            .await;
            for (manifest, read) in manifests.iter().zip(documents) {
                match read
                    .and_then(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
                {
                    Ok(document) => plugins.add(manifest, &document),
                    Err(error) => plugins.error(manifest, error),
                }
            }
        }
        Err(error) => plugins.error(INDEX, error),
    }
    plugins.settle();
    plugins
}

pub(crate) fn manifests() -> Vec<Arc<PluginManifest>> {
    plugins().manifests.clone()
}

pub(crate) fn entry_point(plugin_id: &str, entry: &str) -> Option<String> {
    plugins()
        .manifests
        .iter()
        .any(|manifest| manifest.identity.id == plugin_id)
        .then(|| entry.to_owned())
}

const DATA: &str = "data";
const DATA_INDEX: &str = "index.json";
const DATA_FILES: &str = "files";

pub(crate) fn data_listing(
    plugin_id: &str,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    staged(plugin_id, DATA_INDEX, deliver);
}

pub(crate) fn data(
    plugin_id: &str,
    path: &str,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    if !plain(path) {
        return deliver(Err(format!("{path:?} is not a plain relative path")));
    }
    staged(plugin_id, &format!("{DATA_FILES}/{path}"), deliver);
}

fn staged(
    plugin_id: &str,
    path: &str,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    let plugins = plugins();
    if !plain(plugin_id)
        || !plugins
            .manifests
            .iter()
            .any(|manifest| manifest.identity.id == plugin_id)
    {
        return deliver(Err(format!("{plugin_id} is not a plugin this app found")));
    }
    let relative = format!("{DATA}/{plugin_id}/{path}");
    fetch(relative.clone(), move |read| {
        deliver(read.map_err(|error| format!("{relative}: {error}")));
    });
}

fn plain(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        })
}
