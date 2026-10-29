use super::*;

#[test]
fn manifest_validation() {
    let editor = EditorManifest {
        id: [2; 16],
        name: "counter".into(),
        edits: Edits::Type([1; 16]),
        display_name: "Counter".into(),
        icon: "123".into(),
        templates: vec![TemplateManifest {
            id: "main".into(),
            name: "Counter".into(),
            icon: "123".into(),
            category: TemplateCategory::Regular,
            debug: true,
            dialog: false,
            block_type: [1; 16],
        }],
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::Both,
        views: vec![ParentView::Main, ParentView::Preview],
        accepts: Vec::new(),
    };
    let manifest = PluginManifest {
        identity: PluginIdentity {
            id: "be3.counter".into(),
            name: "Counter".into(),
            version: "1".into(),
        },
        editors: vec![editor.clone()],
        block_types: Vec::new(),
        entry_point: "counter.wasm".into(),
        network: vec!["api.github.com".into()],
        extensions: Vec::new(),
    };
    assert_eq!(manifest.validate(), Ok(()));

    let mut invalid = manifest.clone();
    invalid.editors[0].views.push(ParentView::Preview);
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidViews));

    let mut invalid = manifest.clone();
    invalid.editors[0].views = vec![ParentView::Preview];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidViews));

    let mut invalid = manifest.clone();
    invalid.editors.clear();
    assert_eq!(invalid.validate(), Err(ManifestError::NoEditors));

    let mut invalid = manifest.clone();
    invalid.editors.push(editor.clone());
    assert_eq!(invalid.validate(), Err(ManifestError::DuplicateEditor));

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
