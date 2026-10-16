use super::*;

struct Tray;

impl HostValue for Tray {
    const KEY: &'static str = "test.tray";
    type Value = Vec<u32>;
}

#[test]
fn a_host_value_is_offered_once_the_host_publishes_it() {
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let _root = RootScope::new();
    let offered = editor.host_offers::<Tray>();
    editor.begin_frame();
    assert!(
        !offered.get_untracked(),
        "a host that never published the value does not offer it"
    );

    host.set_host_value::<Tray>(&Vec::new());
    editor.begin_frame();
    assert!(
        offered.get_untracked(),
        "an empty value is still offered, unlike one never published"
    );
}
