use std::sync::Arc;

use block_plugin_api::{Edits, PluginIdentity};
use block_plugin_manifest::{EditorManifest, PluginManifest};

use super::*;

fn editor(name: &str, edits: Edits) -> EditorManifest {
    EditorManifest {
        id: Uuid::new_v4().into_bytes(),
        name: name.to_owned(),
        edits,
        display_name: name.to_owned(),
        icon: String::new(),
        templates: Vec::new(),
        interaction: Default::default(),
        capabilities: Default::default(),
        resize: Default::default(),
        views: Vec::new(),
        accepts: Vec::new(),
    }
}

fn plugin(id: &str, editors: Vec<EditorManifest>) -> Arc<PluginManifest> {
    Arc::new(PluginManifest {
        identity: PluginIdentity {
            id: id.to_owned(),
            name: id.to_owned(),
            version: "1".to_owned(),
        },
        editors,
        block_types: Vec::new(),
        entry_point: String::new(),
        network: Vec::new(),
        extensions: Vec::new(),
    })
}

#[test]
fn a_child_asking_for_another_editor_reopens_in_it() {
    let block_type = Uuid::new_v4();
    let built_in = plugin(
        "test.built-in",
        vec![
            editor("main", Edits::Type(block_type.into_bytes())),
            editor("outline", Edits::Type(block_type.into_bytes())),
        ],
    );
    let debug = plugin("test.debug", vec![editor("debug", Edits::Any)]);
    let (built_in_id, outline_id, debug_id) = (
        Uuid::from_bytes(built_in.editors[0].id),
        Uuid::from_bytes(built_in.editors[1].id),
        Uuid::from_bytes(debug.editors[0].id),
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
        "a block opens in the first editor of its type"
    );

    editors.ensure(id, block_type, None, Some(outline_id), parent);
    assert_eq!(
        opened(&editors),
        Some(outline_id),
        "a second editor of the same type in the same plugin is opened by its id"
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
