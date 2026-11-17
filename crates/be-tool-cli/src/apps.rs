//! What `be` knows of block types from the apps' manifests, which it reads as be-host does,
//! through `be_host_stack::discovery` (`crate::release`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once, PoisonError};

use be_sdk_manifest::BlockTypeManifest;
use uuid::Uuid;

static NAMES: Mutex<Option<HashMap<Uuid, &'static str>>> = Mutex::new(None);

/// The apps discovery found; the first call, whichever command needs them first, reports
/// what discovery could not read (a deployed `be` whose release did not answer, a manifest
/// that did not parse), once.
pub fn found() -> Arc<be_host_stack::discovery::Apps> {
    static REPORTED: Once = Once::new();
    let apps = be_host_stack::discovery::apps();
    REPORTED.call_once(|| {
        for error in apps.errors() {
            eprintln!("be: {error}");
        }
    });
    apps
}

/// The block type `content_type` as an app's manifest declares it.
fn declared<T>(content_type: Uuid, read: impl FnOnce(&BlockTypeManifest) -> T) -> Option<T> {
    found()
        .manifests()
        .iter()
        .flat_map(|manifest| &manifest.block_types)
        .find(|block_type| Uuid::from_bytes(block_type.content_type()) == content_type)
        .map(read)
}

/// The name an app's manifest gives `content_type`, in lower case.
pub fn name_of(content_type: Uuid) -> Option<&'static str> {
    let mut names = NAMES.lock().unwrap_or_else(PoisonError::into_inner);
    let names = names.get_or_insert_with(HashMap::new);
    if let Some(name) = names.get(&content_type) {
        return Some(name);
    }
    let name: &'static str = Box::leak(
        declared(content_type, |block_type| {
            block_type.descriptor.display_name.to_lowercase()
        })?
        .into_boxed_str(),
    );
    names.insert(content_type, name);
    Some(name)
}

/// Whether a block of `content_type` takes children people add.
pub fn adds_children(content_type: Uuid) -> bool {
    declared(content_type, |block_type| block_type.descriptor.children.add).unwrap_or(false)
}
