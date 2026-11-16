use super::*;

/// `be` carries every product app whose manifest declares `hosts`, and only those: the BUCK's
/// `HOST_APPS` is a hand list, so an app that gains a host component and is left off it would
/// silently never run in `be`. Examples (`be-app-example-*`) are the tests' own.
#[test]
fn be_carries_every_product_app_that_hosts_a_type() {
    let packed = std::fs::read(std::env::var("PRODUCT_HOST_APPS").expect("the BUCK names them"))
        .expect("the packed product host apps read");
    let files = crate::hosting::unpack(Vec::leak(packed)).expect("the pack parses");
    let listed: Vec<String> = serde_json::from_slice(files["apps.json"]).expect("apps.json lists");
    let mut carried = std::collections::BTreeSet::new();
    for name in &listed {
        let document = std::str::from_utf8(files[name.as_str()]).expect("the manifest is text");
        let manifest = be_sdk_manifest::manifest_from_json(document).expect("the manifest parses");
        assert!(
            !manifest.hosts.is_empty(),
            "be carries {}, which hosts no type",
            manifest.identity.id
        );
        carried.insert(manifest.identity.id);
    }
    let editors = std::env::var_os("EDITOR_MANIFESTS")
        .map(PathBuf::from)
        .expect("EDITOR_MANIFESTS names the editors' manifests");
    for entry in std::fs::read_dir(&editors).expect("EDITOR_MANIFESTS is a directory") {
        let entry = entry.expect("the entry is readable");
        if entry.file_name().to_string_lossy().starts_with("be-app-example-") {
            continue;
        }
        let document = std::fs::read_to_string(entry.path().join("manifest.json"))
            .expect("the manifest is readable");
        let manifest = be_sdk_manifest::manifest_from_json(&document).expect("the manifest parses");
        if !manifest.hosts.is_empty() {
            assert!(
                carried.contains(&manifest.identity.id),
                "{} hosts a type but is not in HOST_APPS in crates/be-tool-cli/BUCK",
                manifest.identity.id
            );
        }
    }
}
