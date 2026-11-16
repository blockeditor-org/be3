use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, RwLock},
};

use be_sdk_api::BlockTypeDescriptor;
use be_sdk_manifest::AppManifest;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
mod bundle;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use bundle::open;
#[cfg(not(target_arch = "wasm32"))]
pub use bundle::use_assets;
#[cfg(not(target_arch = "wasm32"))]
use bundle::{fetch, scan};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{adopt, fetched};
#[cfg(target_arch = "wasm32")]
pub use web::load;
#[cfg(target_arch = "wasm32")]
use web::{fetch, scan};

const INDEX: &str = "apps.json";

pub struct Apps {
    manifests: Vec<Arc<AppManifest>>,
    types: HashMap<Uuid, BlockTypeDescriptor>,
    errors: Vec<String>,
}

impl Default for Apps {
    fn default() -> Self {
        Self {
            manifests: Vec::new(),
            types: block_types(&[]),
            errors: Vec::new(),
        }
    }
}

pub fn block_types(manifests: &[Arc<AppManifest>]) -> HashMap<Uuid, BlockTypeDescriptor> {
    crate::host_block_types()
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

impl Apps {
    pub fn manifests(&self) -> &[Arc<AppManifest>] {
        &self.manifests
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    fn add(&mut self, source: &str, document: &str) {
        let manifest = match be_sdk_manifest::manifest_from_json(document) {
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
                "{source}: {block_type} is a block type the host or an app found first already provides"
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
                "{source}: {} is already the id of an app that was found first",
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
                "{}: {missing} is not a block type the host or any app provides",
                manifest.identity.id
            ));
            self.types = block_types(&self.manifests);
        }
    }

    fn unprovided(manifest: &AppManifest) -> impl Iterator<Item = Uuid> + '_ {
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

static APPS: RwLock<Option<Arc<Apps>>> = RwLock::new(None);

pub fn apps() -> Arc<Apps> {
    if let Some(apps) = APPS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return Arc::clone(apps);
    }
    let mut held = APPS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Arc::clone(held.get_or_insert_with(|| Arc::new(scan())))
}

/// Whether `load` installed the apps in this memory.
#[cfg(target_arch = "wasm32")]
pub(crate) fn loaded() -> bool {
    APPS.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
}

#[cfg(target_arch = "wasm32")]
fn install(apps: Apps) {
    *APPS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(apps));
}

/// Puts these manifests, `(source, document)`, in place of the apps found, for a test whose host
/// stages none, until the answer is dropped: the apps found are the process's, which other tests
/// in it share. Fails on one discovery would refuse.
#[cfg(any(test, feature = "testing"))]
#[must_use = "the manifests are in place only while this is held"]
pub fn install_for_testing(manifests: &[(&str, &str)]) -> InstalledForTesting {
    let mut apps = Apps::default();
    for (source, document) in manifests {
        apps.add(source, document);
    }
    apps.settle();
    assert!(apps.errors.is_empty(), "{:?}", apps.errors);
    let before = APPS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(Arc::new(apps));
    InstalledForTesting(before)
}

/// Manifests a test put in place (`install_for_testing`), until it is dropped.
#[cfg(any(test, feature = "testing"))]
pub struct InstalledForTesting(Option<Arc<Apps>>);

#[cfg(any(test, feature = "testing"))]
impl Drop for InstalledForTesting {
    fn drop(&mut self) {
        *APPS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.0.take();
    }
}

pub fn block_type(content_type: Uuid) -> Option<BlockTypeDescriptor> {
    apps().types.get(&content_type).cloned()
}

pub fn exported_whole(content_type: Uuid) -> bool {
    apps()
        .manifests
        .iter()
        .flat_map(|manifest| &manifest.block_types)
        .any(|block_type| {
            block_type.exported_whole && Uuid::from_bytes(block_type.content_type()) == content_type
        })
}

/// What a start read to find its apps: the build's id, and each manifest the index names with
/// what reading it gave, or why the index could not be read. On the web the page reads it once
/// and hands it to its host's worker as a stack starts there (`web::fetched`), since the two
/// share no memory.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Read {
    build_id: Option<String>,
    manifests: Result<Vec<(String, Result<String, String>)>, String>,
}

async fn fetch_all<F>(read: impl Fn(String) -> F) -> Read
where
    F: Future<Output = Result<Vec<u8>, String>>,
{
    let (id, index) = futures_util::join!(
        read(be_lib_build_id::FILE.to_owned()),
        read(INDEX.to_owned())
    );
    let index = index.and_then(|document| {
        serde_json::from_slice::<Vec<String>>(&document).map_err(|error| error.to_string())
    });
    let manifests = match index {
        Ok(manifests) => {
            let documents = futures_util::future::join_all(
                manifests.iter().map(|manifest| read(manifest.clone())),
            )
            .await;
            Ok(manifests
                .into_iter()
                .zip(documents)
                .map(|(manifest, read)| {
                    let document = read.and_then(|bytes| {
                        String::from_utf8(bytes).map_err(|error| error.to_string())
                    });
                    (manifest, document)
                })
                .collect())
        }
        Err(error) => Err(error),
    };
    Read {
        build_id: id.ok().map(|id| String::from_utf8_lossy(&id).into_owned()),
        manifests,
    }
}

/// The apps `read` found, and the build's id installed.
fn settle(read: &Read) -> Apps {
    if let Some(id) = &read.build_id {
        let _ = be_lib_build_id::install_metadata(id);
    }
    let mut apps = Apps::default();
    match &read.manifests {
        Ok(manifests) => {
            for (manifest, document) in manifests {
                match document {
                    Ok(document) => apps.add(manifest, document),
                    Err(error) => apps.error(manifest, error),
                }
            }
        }
        Err(error) => apps.error(INDEX, error),
    }
    apps.settle();
    apps
}

pub fn manifests() -> Vec<Arc<AppManifest>> {
    apps().manifests.clone()
}

pub fn entry_point(app_id: &str, entry: &str) -> Option<String> {
    apps()
        .manifests
        .iter()
        .any(|manifest| manifest.identity.id == app_id)
        .then(|| entry.to_owned())
}

const DATA: &str = "data";
const DATA_INDEX: &str = "index.json";
const DATA_FILES: &str = "files";

pub fn data_listing(app_id: &str, deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    staged(app_id, DATA_INDEX, deliver);
}

pub fn data(
    app_id: &str,
    path: &str,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    if !plain(path) {
        return deliver(Err(format!("{path:?} is not a plain relative path")));
    }
    staged(app_id, &format!("{DATA_FILES}/{path}"), deliver);
}

fn staged(
    app_id: &str,
    path: &str,
    deliver: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    let apps = apps();
    if !plain(app_id)
        || !apps
            .manifests
            .iter()
            .any(|manifest| manifest.identity.id == app_id)
    {
        return deliver(Err(format!("{app_id} is not an app this host found")));
    }
    let relative = format!("{DATA}/{app_id}/{path}");
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
