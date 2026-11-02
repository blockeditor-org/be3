use super::*;

#[test]
fn the_mode_dock_switches_the_profile() {
    let mut editor = editor(&[]).with_top_bar(false);
    let profile =
        |editor: &BeuiTest<CanvasApp>| editor.content::<CanvasContent>(None).root().profile;
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let mode = editor.rect_of("infinite-canvas.mode");
    assert!(
        mode.left() - canvas.left() < 24.0 && mode.top() - canvas.top() < 24.0,
        "the mode dock sits in the top-left corner, at {mode:?} in {canvas:?}"
    );
    let zoom = editor.rect_of("infinite-canvas.zoom-pill");
    assert!(
        mode.right() < zoom.left(),
        "the mode dock keeps clear of the zoom pill"
    );
    assert!(pressed(&editor, "infinite-canvas.mode.prototyping"));
    assert!(!pressed(&editor, "infinite-canvas.mode.raster"));
    assert!(!editor.shown("infinite-canvas.dock.tool.pencil"));

    editor.click("infinite-canvas.mode.raster");
    editor.run();
    assert_eq!(profile(&editor), CanvasProfile::Raster);
    assert!(pressed(&editor, "infinite-canvas.mode.raster"));
    assert!(!pressed(&editor, "infinite-canvas.mode.prototyping"));
    assert!(editor.shown("infinite-canvas.dock.tool.pencil"));
    assert!(
        !editor.shown("infinite-canvas.dock.tool.Rectangle"),
        "the raster profile draws with pixel tools"
    );
    assert!(
        editor.shown("infinite-canvas.raster"),
        "and has its own panel"
    );
    assert!(editor.menu_entry("canvas.profile.raster").enabled);
    editor.snapshot("the_mode_dock_switches_the_profile");

    editor.click("infinite-canvas.mode.vector");
    editor.run();
    assert_eq!(profile(&editor), CanvasProfile::Vector);
    assert!(editor.shown("infinite-canvas.selection"));
    assert!(!editor.shown("infinite-canvas.raster"));

    editor.pick_menu("canvas.profile.raster");
    editor.run();
    assert!(
        pressed(&editor, "infinite-canvas.mode.raster"),
        "the dock follows the Profile menu"
    );

    let mut phone = phone(&[]);
    assert!(
        !phone.shown("infinite-canvas.mode"),
        "a phone has no room for it beside the zoom pill"
    );
    phone.click("infinite-canvas.mode.menu");
    phone.run();
    phone.click("infinite-canvas.mode.canvas.profile.raster");
    phone.run();
    assert_eq!(profile(&phone), CanvasProfile::Raster);
    assert!(phone.shown("infinite-canvas.dock.tool.pencil"));
}
