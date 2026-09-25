use super::*;

use block_editor_plugin::beui::reactive::{Frame, build, with_reactive_scope};

use crate::app::state::RayState;

#[test]
fn a_traced_frame_lands_without_asking_to_be_stepped_again() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let store = block_ui_test::ContentStore::new(host.clone());
    store.hold(None, PixelRayTracerContent::default());
    let mut document = build(|| Frame().build());

    let state = with_reactive_scope(&mut document, || {
        let state = RayState::new(&editor);
        state.watch();
        state
    });
    for _ in 0..2 {
        with_reactive_scope(&mut document, || editor.begin_frame());
    }

    assert!(
        state.lighting.get_untracked().is_some(),
        "the lighting the tracer finished was never shown"
    );
    assert!(
        host.take_frame_request().is_none(),
        "the editor asked to be stepped again instead of waiting to be woken"
    );
}
