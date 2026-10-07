use std::sync::Arc;

use block_plugin_api::{EditorManifest, PluginIdentity, PluginManifest};

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
                block_type: block_type.into_bytes(),
                display_name: "Test".to_owned(),
                icon: String::new(),
                templates: Vec::new(),
                children: Default::default(),
                interaction: Default::default(),
                capabilities: Default::default(),
                resize: Default::default(),
                regions: Vec::new(),
            }],
            entry_point: String::new(),
            network: Vec::new(),
        });
        let registry = Rc::new(EditorRegistry::from_manifests(vec![plugin]));
        let editors = Editors::install(registry, Uuid::new_v4());
        let id = Uuid::new_v4();
        editors.ensure(id, block_type, None);
        editors.with(|open| open.get_mut(&id).expect("the editor is open").shown(true));

        drop(editors);

        assert!(
            EDITORS.with(|held| held.borrow().upgrade().is_none()),
            "an editor left to the thread's teardown is dropped after the plugin host it closes \
             through, which aborts the app when it exits",
        );
    })
    .join()
    .expect("the editors closed without panicking");
}
