use super::*;

#[test]
fn every_type_without_a_plugin_module_is_one_this_module_has() {
    let editors = std::env::var_os("EDITOR_MANIFESTS")
        .map(std::path::PathBuf::from)
        .expect("EDITOR_MANIFESTS names the editors' manifests");
    let mut declared: HashMap<Uuid, (String, bool)> = HashMap::new();
    for entry in std::fs::read_dir(&editors).expect("EDITOR_MANIFESTS is a directory") {
        let path = entry
            .expect("the entry is readable")
            .path()
            .join("manifest.json");
        let document = std::fs::read_to_string(&path).expect("the manifest is readable");
        let manifest = manifest_from_json(&document)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for block_type in &manifest.block_types {
            let content_type = Uuid::from_bytes(block_type.content_type());
            let held = (manifest.identity.id.clone(), block_type.module.is_some());
            if let Some((first, _)) = declared.insert(content_type, held) {
                panic!(
                    "{content_type} is declared by {first} and {}",
                    manifest.identity.id
                );
            }
        }
    }
    let host: Vec<Uuid> = host()
        .iter()
        .map(|registration| registration.info().content_type)
        .collect();
    for content_type in &host {
        assert!(
            !declared.contains_key(content_type),
            "{content_type} is applied by the app, but a plugin declares it too"
        );
    }
    let ours: Vec<Uuid> = registrations()
        .iter()
        .map(|registration| registration.info().content_type)
        .collect();
    for content_type in &ours {
        match declared.get(content_type) {
            None => panic!("{content_type} is in builtin-types, but no plugin declares it"),
            Some((plugin, true)) => {
                panic!(
                    "{plugin} brings {content_type} in its own module; take it out of builtin-types"
                )
            }
            Some((_, false)) => {}
        }
    }
    for (content_type, (plugin, module)) in &declared {
        assert!(
            *module || ours.contains(content_type),
            "{plugin} declares {content_type} with no module of its own, and builtin-types does not have it"
        );
    }
}
