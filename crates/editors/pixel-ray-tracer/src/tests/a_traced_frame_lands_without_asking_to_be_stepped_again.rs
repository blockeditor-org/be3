use super::*;

use block_editor_beui::beui::reactive::*;

use block_editor_beui::HostContent;

use crate::app::state::RayState;

#[test]
fn a_traced_frame_lands_without_asking_to_be_stepped_again() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), block);
    host.set_block_content(HostContent::of(&PixelRayTracerContent::default()));
    let _root = RootScope::new();

    let state = RayState::new(*editor);
    state.watch();
    flush();
    for _ in 0..2 {
        editor.begin_frame();
        flush();
    }

    assert!(
        state.lighting.get_untracked().is_some(),
        "the lighting the tracer finished was never shown"
    );
    assert!(
        host.take_frame_request(block_editor_beui::ViewId::MAIN)
            .is_none(),
        "the editor asked to be stepped again instead of waiting to be woken"
    );
}
