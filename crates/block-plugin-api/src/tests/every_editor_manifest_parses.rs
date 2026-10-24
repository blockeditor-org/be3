use super::*;

#[test]
fn every_editor_manifest_parses() {
    let editors = std::env::var_os("EDITOR_MANIFESTS")
        .map(std::path::PathBuf::from)
        .expect("EDITOR_MANIFESTS names the editors' manifests");
    let mut checked = 0;
    let mut declared = std::collections::HashMap::new();
    for entry in std::fs::read_dir(&editors).expect("EDITOR_MANIFESTS is a directory") {
        let manifest = entry
            .expect("the entry is readable")
            .path()
            .join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let document = std::fs::read_to_string(&manifest).expect("the manifest is readable");
        let parsed = manifest_from_json(&document)
            .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
        for block_type in &parsed.block_types {
            if let Some(first) =
                declared.insert(block_type.content_type(), parsed.identity.id.clone())
            {
                panic!(
                    "{} is declared by {first} and {}",
                    uuid::Uuid::from_bytes(block_type.content_type()),
                    parsed.identity.id
                );
            }
        }
        checked += 1;
    }
    assert!(checked > 0, "no editor manifests were found");
}
