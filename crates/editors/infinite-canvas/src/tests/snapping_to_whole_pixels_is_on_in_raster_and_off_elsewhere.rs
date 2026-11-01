use super::*;
use block_editor_beui::MenuCheck;

fn snaps(editor: &BeuiTest<CanvasApp>) -> bool {
    editor.menu_entry("canvas.snap").check == MenuCheck::Checkbox(true)
}

fn corner(editor: &BeuiTest<CanvasApp>) -> CanvasPoint {
    entity_bounds(&entities(editor)[0]).min
}

#[test]
fn snapping_to_whole_pixels_is_on_in_raster_and_off_elsewhere() {
    let mut shape = placed(1, CanvasShape::Rectangle, CanvasPoint::new(0.25, 0.25));
    shape.style.fill = CanvasFill::Solid;
    let mut editor = vector_editor(std::slice::from_ref(&shape)).with_top_bar(false);
    assert!(
        !snaps(&editor),
        "the vector profile starts without snapping"
    );
    let center = editor.rect_of("infinite-canvas.canvas").center();
    let step = Vec2::new(10.3, 5.2);

    editor.drag(center, center + step);
    editor.run();
    assert!(
        !snaps(&editor),
        "the vector profile starts without snapping"
    );
    let moved = corner(&editor);
    assert!(
        (moved.x - (-59.75 + 10.3)).abs() < 0.01 && (moved.y - (-39.75 + 5.2)).abs() < 0.01,
        "a move follows the pointer exactly, to {moved:?}"
    );

    editor.pick_menu("canvas.snap");
    editor.run();
    assert!(snaps(&editor));
    editor.drag(center + step, center + step * 2.0);
    editor.run();
    let snapped = corner(&editor);
    assert_eq!(
        snapped,
        CanvasPoint::new(snapped.x.round(), snapped.y.round()),
        "with snapping on, a move lands on whole units"
    );
    assert!((snapped.x - moved.x - 10.3).abs() <= 0.5);

    editor.click("infinite-canvas.mode.prototyping");
    editor.run();
    assert!(!snaps(&editor), "each profile keeps its own choice");
    editor.click("infinite-canvas.mode.raster");
    editor.run();
    assert!(snaps(&editor), "the raster profile starts snapping");
    assert!(pressed(&editor, "infinite-canvas.raster.snap"));
    editor.click("infinite-canvas.raster.snap");
    editor.run();
    assert!(!snaps(&editor));
    editor.click("infinite-canvas.mode.vector");
    editor.run();
    assert!(
        snaps(&editor),
        "the vector profile kept the choice made in it"
    );
}
