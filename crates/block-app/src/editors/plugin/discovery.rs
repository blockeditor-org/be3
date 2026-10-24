use std::{cell::RefCell, future::Future, sync::Arc};

use block_plugin_api::PluginManifest;
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

#[derive(Default)]
pub(crate) struct Plugins {
    manifests: Vec<Arc<PluginManifest>>,
    errors: Vec<String>,
}

impl Plugins {
    pub(crate) fn manifests(&self) -> &[Arc<PluginManifest>] {
        &self.manifests
    }

    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
    }

    fn add(&mut self, source: &str, document: &str) {
        let manifest = match block_plugin_api::manifest_from_json(document) {
            Ok(manifest) => manifest,
            Err(error) => {
                self.errors.push(format!("{source}: {error}"));
                return;
            }
        };
        for block_type in manifest.editors.iter().flat_map(|editor| {
            editor
                .edits
                .block_type()
                .into_iter()
                .chain(editor.templates.iter().map(|template| template.block_type))
        }) {
            let block_type = Uuid::from_bytes(block_type);
            if be_block::block_type(block_type).is_none() {
                self.errors.push(format!(
                    "{source}: {block_type} is not a block type this app has"
                ));
                return;
            }
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
        self.manifests.push(Arc::new(manifest));
    }

    fn error(&mut self, source: &str, error: impl std::fmt::Display) {
        self.errors.push(format!("{source}: {error}"));
    }
}

thread_local! {
    static PLUGINS: RefCell<Option<Arc<Plugins>>> = const { RefCell::new(None) };
}

pub(crate) fn plugins() -> Arc<Plugins> {
    PLUGINS.with(|cell| {
        let mut cell = cell.borrow_mut();
        Arc::clone(cell.get_or_insert_with(|| Arc::new(scan())))
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
