use super::*;

#[test]
fn manifest_validation() {
    let editor = EditorManifest {
        block_type: [1; 16],
        display_name: "Counter".into(),
        icon: "123".into(),
        templates: vec![TemplateManifest {
            id: "main".into(),
            name: "Counter".into(),
            icon: "123".into(),
            category: TemplateCategory::Debug,
            dialog: false,
            block_type: [1; 16],
        }],
        children: ChildOperations::default(),
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::Both,
        regions: vec![EditorRegion::Frame, EditorRegion::Preview],
    };
    let manifest = PluginManifest {
        identity: PluginIdentity {
            id: "be3.counter".into(),
            name: "Counter".into(),
            version: "1".into(),
        },
        editors: vec![editor.clone()],
        entry_point: "counter.wasm".into(),
        network: vec!["api.github.com".into()],
    };
    assert_eq!(manifest.validate(), Ok(()));

    let mut invalid = manifest.clone();
    invalid.editors[0].regions.push(EditorRegion::Preview);
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest.clone();
    invalid.editors[0].regions = vec![EditorRegion::Preview];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest.clone();
    invalid.editors.clear();
    assert_eq!(invalid.validate(), Err(ManifestError::NoEditors));

    let mut invalid = manifest.clone();
    invalid.editors.push(editor.clone());
    assert_eq!(invalid.validate(), Err(ManifestError::DuplicateBlockType));

    let mut invalid = manifest.clone();
    let template = invalid.editors[0].templates[0].clone();
    invalid.editors[0].templates.push(template);
    assert_eq!(invalid.validate(), Err(ManifestError::DuplicateTemplate));

    let mut invalid = manifest.clone();
    invalid.entry_point = String::new();
    assert_eq!(invalid.validate(), Err(ManifestError::Empty("entry point")));

    let mut invalid = manifest.clone();
    invalid.network = vec!["https://api.github.com/".into()];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidNetworkHost));

    let mut invalid = manifest;
    invalid.network = vec![String::new()];
    assert_eq!(
        invalid.validate(),
        Err(ManifestError::Empty("network host"))
    );
}
