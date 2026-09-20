use std::sync::Arc;

use block_client::blocks::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_BACKGROUND, PixelRayTracer, PixelRayTracerOperation, PixelUpdate,
};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PixelRayTracerApp;

mod a_new_scene_paints_nothing_until_the_lighting_lands;
mod a_settled_editor_stops_laying_itself_out_again;
mod an_editor_waiting_on_a_traced_frame_asks_to_be_stepped_again;
mod resetting_the_artwork_clears_painted_pixels;
mod zooming_the_view_grows_the_scene;

fn editor() -> (
    BeuiTest<PixelRayTracerApp>,
    BlockHandle<PixelRayTracer>,
    EditorHost,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(PixelRayTracer::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.settle_until("the lighting to land", |editor| {
        editor.shown("pixel_ray_tracer.artwork") && !editor.wants_another_frame()
    });
    (editor, block, host)
}
