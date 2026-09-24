use super::*;

use block_editor_plugin::beui::reactive::{Frame, build, with_reactive_scope};

use crate::app::state::RayState;

#[test]
fn an_editor_waiting_on_a_traced_frame_asks_to_be_stepped_again() {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(PixelRayTracer);
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let store = block_ui_test::ContentStore::new(host.clone());
    store.hold(None, PixelRayTracerContent::default());
    let mut document = build(|| Frame().build());

    with_reactive_scope(&mut document, || {
        let state = RayState::new(&editor);
        state.poll();
    });

    assert!(
        host.take_frame_request().is_some(),
        "a tracing job only lands when the editor is stepped again, and nothing else will ask"
    );
}
