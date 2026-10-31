use std::sync::Arc;

use block_plugin_api::PluginIdentity;
use block_plugin_manifest::{EditorManifest, PluginManifest};

use super::*;

#[test]
fn plugin_editors_close_with_their_owner_rather_than_when_the_thread_ends() {
    std::thread::spawn(|| {
        let block_type = Uuid::new_v4();
        let plugin = Arc::new(PluginManifest {
            identity: PluginIdentity {
                id: "test.plugin".to_owned(),
                name: "Test".to_owned(),
                version: "1".to_owned(),
            },
            editors: vec![EditorManifest {
                id: Uuid::new_v4().into_bytes(),
                name: "test".to_owned(),
                edits: block_plugin_api::Edits::Type(block_type.into_bytes()),
                display_name: "Test".to_owned(),
                icon: String::new(),
                templates: Vec::new(),
                interaction: Default::default(),
                capabilities: Default::default(),
                resize: Default::default(),
                views: Vec::new(),
                accepts: Vec::new(),
                opens: Vec::new(),
            }],
            block_types: Vec::new(),
            entry_point: String::new(),
            network: Vec::new(),
            extensions: Vec::new(),
        });
        let registry = Rc::new(EditorRegistry::from_manifests(vec![plugin]));
        let editors = OwnedEditors::install(registry, Uuid::new_v4());
        let id = Uuid::new_v4();
        editors.ensure(id, block_type, None, None, Uuid::new_v4());
        editors.with(|open| open.get_mut(&id).expect("the editor is open").shown(true));

        drop(editors);

        assert!(
            EDITORS.get().is_none_or(|editors| !editors.0.is_alive()),
            "an editor left to the thread's teardown is dropped after the plugin host it closes \
             through, which aborts the app when it exits",
        );
    })
    .join()
    .expect("the editors closed without panicking");
}
