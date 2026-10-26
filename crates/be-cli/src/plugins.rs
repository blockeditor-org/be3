use std::{collections::HashMap, sync::OnceLock};

use block_plugin_api::{PluginManifest, manifest_from_json};
use uuid::Uuid;

pub struct Plugin {
    pub manifest: PluginManifest,
    pub module: &'static [u8],
}

impl Plugin {
    pub fn declared(&self) -> Vec<Uuid> {
        self.manifest
            .block_types
            .iter()
            .map(|block_type| Uuid::from_bytes(block_type.content_type()))
            .collect()
    }
}

static PLUGINS: OnceLock<Vec<Plugin>> = OnceLock::new();

static NAMES: OnceLock<HashMap<Uuid, &'static str>> = OnceLock::new();

fn take(rest: &mut &'static [u8]) -> Result<&'static [u8], String> {
    let (length, after) = rest
        .split_at_checked(8)
        .ok_or("the plugin bundle ends inside a length")?;
    let length = std::str::from_utf8(length)
        .ok()
        .and_then(|length| usize::from_str_radix(length, 16).ok())
        .ok_or("the plugin bundle holds a length that is not hex")?;
    let (record, after) = after
        .split_at_checked(length)
        .ok_or("the plugin bundle ends inside a record")?;
    *rest = after;
    Ok(record)
}

pub fn parse(bundle: &'static [u8]) -> Result<Vec<Plugin>, String> {
    let mut rest = bundle;
    let mut plugins = Vec::new();
    while !rest.is_empty() {
        let manifest = std::str::from_utf8(take(&mut rest)?).map_err(|error| error.to_string())?;
        let manifest = manifest_from_json(manifest).map_err(|error| error.to_string())?;
        let module = take(&mut rest)?;
        plugins.push(Plugin { manifest, module });
    }
    Ok(plugins)
}

pub fn install(plugins: Vec<Plugin>) -> &'static [Plugin] {
    PLUGINS.get_or_init(|| plugins)
}

pub fn installed() -> &'static [Plugin] {
    PLUGINS.get().map_or(&[], Vec::as_slice)
}

pub fn name_of(content_type: Uuid) -> Option<&'static str> {
    NAMES
        .get_or_init(|| {
            installed()
                .iter()
                .flat_map(|plugin| &plugin.manifest.block_types)
                .map(|block_type| {
                    let name: &'static str = Box::leak(
                        block_type
                            .descriptor
                            .display_name
                            .to_lowercase()
                            .into_boxed_str(),
                    );
                    (Uuid::from_bytes(block_type.content_type()), name)
                })
                .collect()
        })
        .get(&content_type)
        .copied()
}

pub fn adds_children(content_type: Uuid) -> bool {
    installed()
        .iter()
        .flat_map(|plugin| &plugin.manifest.block_types)
        .any(|block_type| {
            block_type.descriptor.children.add
                && Uuid::from_bytes(block_type.content_type()) == content_type
        })
}
