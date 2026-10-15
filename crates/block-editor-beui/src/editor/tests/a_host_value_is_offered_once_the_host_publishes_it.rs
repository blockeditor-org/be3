use super::*;

use beui::reactive::{Frame, build, with_reactive_scope};

struct Tray;

impl HostValue for Tray {
    const KEY: &'static str = "test.tray";
    type Value = Vec<u32>;
}

#[test]
fn a_host_value_is_offered_once_the_host_publishes_it() {
    let host = EditorHost::default();
    let editor = OwnedEditor::new(host.clone(), Uuid::new_v4());
    let mut document = build(|| Frame().build());
    let offered = with_reactive_scope(&mut document, || editor.host_offers::<Tray>());
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert!(
        !offered.get_untracked(),
        "a host that never published the value does not offer it"
    );

    host.set_host_value::<Tray>(&Vec::new());
    with_reactive_scope(&mut document, || editor.begin_frame());
    assert!(
        offered.get_untracked(),
        "an empty value is still offered, unlike one never published"
    );
}
