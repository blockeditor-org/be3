use block_editor_beui::be_block::PixelRayTracerContent;
use block_editor_beui::be_block::pixel_ray_tracer::{
    PIXEL_RAY_TRACER_BACKGROUND, PixelRayTracerOperation, PixelUpdate, Scene,
};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PixelRayTracerApp;

mod a_new_scene_paints_nothing_until_the_lighting_lands;
mod a_settled_editor_stops_laying_itself_out_again;
mod a_traced_frame_lands_without_asking_to_be_stepped_again;
mod resetting_the_artwork_clears_painted_pixels;
mod zooming_the_view_grows_the_scene;

fn editor() -> BeuiTest<PixelRayTracerApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.hold(None, PixelRayTracerContent::default());
    editor.settle_until("the lighting to land", |editor| {
        editor.shown("pixel_ray_tracer.artwork") && !editor.wants_another_frame()
    });
    editor
}

fn scene(editor: &BeuiTest<PixelRayTracerApp>) -> Scene {
    editor.content::<PixelRayTracerContent>(None).root().scene()
}

fn operate(editor: &mut BeuiTest<PixelRayTracerApp>, operation: &PixelRayTracerOperation) {
    let edit = editor
        .content::<PixelRayTracerContent>(None)
        .root()
        .edit_for(operation);
    editor.edit::<PixelRayTracerContent>(None, &edit);
}
