use super::*;

#[test]
fn clearing_a_raster_layer_needs_the_dialog() {
    let layer = raster_layer(1, CanvasPoint::default(), 8);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    paint(&mut editor, layer.id, &[(1, 1), (2, 2)], RED);
    editor.click(format!("infinite-canvas.entity.{}", layer.id));
    editor.run();

    assert!(!editor.shown("infinite-canvas.raster.clearing.confirm"));
    editor.click("infinite-canvas.raster.clear");
    editor.run();
    assert!(editor.shown("infinite-canvas.raster.clearing.confirm"));
    editor.click("infinite-canvas.raster.clearing.cancel");
    editor.run();
    assert!(!editor.shown("infinite-canvas.raster.clearing.confirm"));
    assert_eq!(painted(&raster_of(&editor, layer.id)), 2);

    editor.click("infinite-canvas.raster.clear");
    editor.run();
    editor.click("infinite-canvas.raster.clearing.confirm");
    editor.run();
    assert_eq!(painted(&raster_of(&editor, layer.id)), 0);
}
