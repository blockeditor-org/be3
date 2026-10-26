use super::*;

use block_plugin_api::TrayWatch;

#[test]
fn a_watch_is_offered_once_the_host_answers_it() {
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let _root = RootScope::new();
    let offered = editor.offered::<TrayWatch>();
    editor.begin_frame();
    assert!(
        !offered.get_untracked(),
        "a host that never answered the watch does not offer it"
    );

    host.set_watched::<TrayWatch>(&Vec::new());
    editor.begin_frame();
    assert!(
        offered.get_untracked(),
        "an empty value is still offered, unlike one never answered"
    );
}
