use super::*;

/// The raster pencil asks for every sample the way the pen tool does, while it waits for a
/// press: the window learns of it only from the frame after, so a first frame that merged
/// moves would lose pixels of the stroke.
#[test]
fn the_pencil_asks_for_every_sample_before_its_stroke_begins() {
    let mut editor = raster_editor(&[]);
    assert!(
        !editor.window().every_sample,
        "the select tool lets the host merge a frame's moves"
    );

    editor.click("infinite-canvas.dock.tool.pencil");
    assert!(
        editor.window().every_sample,
        "the pencil asks for every sample before it is pressed"
    );

    editor.click("infinite-canvas.dock.tool.fill");
    assert!(
        !editor.window().every_sample,
        "a tool that does not draw a run of samples does not ask"
    );

    editor.click("infinite-canvas.dock.tool.pencil");
    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.drag(from, from + Vec2::new(40.0, 0.0));
    assert_eq!(rasters(&editor).len(), 1, "a stroke paints a layer");
    assert!(
        editor.window().every_sample,
        "the pencil still asks when its stroke is over"
    );
}
