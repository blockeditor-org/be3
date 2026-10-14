use std::sync::Arc;

use block_plugin_api::{ANY_BLOCK_TYPE, EditorManifest, PluginIdentity, PluginManifest};

use super::*;

fn plugin(id: &str, block_type: [u8; 16]) -> Arc<PluginManifest> {
    Arc::new(PluginManifest {
        identity: PluginIdentity {
            id: id.to_owned(),
            name: id.to_owned(),
            version: "1".to_owned(),
        },
        editors: vec![EditorManifest {
            block_type,
            display_name: id.to_owned(),
            icon: String::new(),
            templates: Vec::new(),
            children: Default::default(),
            interaction: Default::default(),
            capabilities: Default::default(),
            resize: Default::default(),
            regions: Vec::new(),
            accepts: Vec::new(),
        }],
        entry_point: String::new(),
        network: Vec::new(),
    })
}

#[test]
fn a_child_asking_for_another_editor_reopens_in_it() {
    let block_type = Uuid::new_v4();
    let built_in = plugin("test.built-in", block_type.into_bytes());
    let debug = plugin("test.debug", ANY_BLOCK_TYPE);
    let (built_in_id, debug_id) = (
        Uuid::from_bytes(built_in.editor_id()),
        Uuid::from_bytes(debug.editor_id()),
    );
    let registry = Rc::new(EditorRegistry::from_manifests(vec![built_in, debug]));
    let editors = OwnedEditors::install(registry, Uuid::new_v4());
    let (id, parent) = (Uuid::new_v4(), Uuid::new_v4());
    let opened = |editors: &OwnedEditors| {
        editors.with(|open| open.get(&id).and_then(PluginEditor::editor_id))
    };

    editors.ensure(id, block_type, None, None, parent);
    assert_eq!(
        opened(&editors),
        Some(built_in_id),
        "a block opens in its type's own editor"
    );

    editors.ensure(id, block_type, None, Some(debug_id), parent);
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "asking for an editor of every type opens it"
    );

    editors.ensure(id, block_type, None, None, parent);
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "a child that names no editor keeps the open one"
    );

    editors.ensure(id, block_type, None, Some(Uuid::new_v4()), parent);
    assert_eq!(
        opened(&editors),
        Some(debug_id),
        "an editor nobody provides is not opened"
    );

    editors.ensure(id, block_type, None, Some(built_in_id), parent);
    assert_eq!(opened(&editors), Some(built_in_id));

    drop(editors);
}
