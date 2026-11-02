use super::*;

const MANIFEST: &str = r#"{
    "id": "be3.checklist",
    "name": "Checklist",
    "version": "0.1.0",
    "block_types": {
        "checklist": {
            "id": "63686563-6b6c-6973-742d-626c6f636b31",
            "display_name": "Checklist",
            "icon": "x",
            "module": "checklist_types.wasm"
        }
    },
    "entry_point": "checklist.wasm"
}"#;

fn record(bytes: &[u8]) -> Vec<u8> {
    let mut record = format!("{:08x}", bytes.len()).into_bytes();
    record.extend_from_slice(bytes);
    record
}

#[test]
fn the_plugin_bundle_holds_each_manifest_and_its_type_module() {
    let mut bundle = record(MANIFEST.as_bytes());
    bundle.extend(record(b"\0asm"));
    bundle.extend(record(
        MANIFEST.replace("be3.checklist", "be3.other").as_bytes(),
    ));
    bundle.extend(record(b""));
    let bundle: &'static [u8] = Box::leak(bundle.into_boxed_slice());

    let plugins = crate::plugins::parse(bundle).expect("the bundle reads");
    assert_eq!(plugins.len(), 2);
    assert_eq!(plugins[0].manifest.identity.id, "be3.checklist");
    assert_eq!(plugins[0].module, b"\0asm");
    assert_eq!(
        plugins[0].declared(),
        [Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31)]
    );
    assert!(plugins[1].module.is_empty());
    assert!(crate::plugins::parse(&bundle[..bundle.len() - 3]).is_err());
}
