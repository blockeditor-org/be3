use super::*;

#[test]
fn a_raster_layer_is_drawn_pixel_sharp_with_its_grid() {
    let layer = raster_layer(1, CanvasPoint::new(0.0, 0.0), 8);
    let mut faded = raster_layer(2, CanvasPoint::new(-10.0, 0.0), 4);
    faded.style.opacity = 0.4;
    let mut editor = raster_editor(&[layer.clone(), faded.clone()]).with_top_bar(false);
    let heart = [
        (1, 1),
        (2, 1),
        (5, 1),
        (6, 1),
        (0, 2),
        (3, 2),
        (4, 2),
        (7, 2),
        (0, 3),
        (7, 3),
        (1, 4),
        (6, 4),
        (2, 5),
        (5, 5),
        (3, 6),
        (4, 6),
    ];
    paint(&mut editor, layer.id, &heart, RED);
    paint(
        &mut editor,
        layer.id,
        &[(2, 2), (5, 2), (3, 3), (4, 3)],
        BLACK,
    );
    paint(
        &mut editor,
        faded.id,
        &[(0, 0), (1, 1), (2, 2), (3, 3)],
        BLACK,
    );

    let corner = editor.rect_of("infinite-canvas.canvas").center();
    editor.pinch(corner, 3.0);
    editor.run();
    editor.settle();
    editor.snapshot("a_raster_layer_is_drawn_pixel_sharp_with_its_grid");
}
