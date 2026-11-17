use super::*;

#[test]
fn drawings_that_place_one_layer_id_each_show_in_a_composited_window() {
    beui_draw::compose::composite_windows();
    let mut target = Target::new();
    target.renderer.set_window(true);
    let screen = be_lib_layers::Surface::Screen {
        instance: 1,
        surface: 0,
    };
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let below = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[red]), screen, 3));
    let above = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[blue]), screen, 3));
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.drawing(Rect::from_min_max(pos2(0.0, 0.0), pos2(16.0, 16.0)), &below);
        painter.drawing(Rect::from_min_max(pos2(32.0, 0.0), pos2(48.0, 16.0)), &above);
    });
    let layers = target.renderer.layers().expect("the window is composited");
    let mut ids: Vec<_> = layers.layers.iter().map(|layer| layer.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3, "each layer has an id of its own: {layers:?}");
    let capture = target.read();
    assert_eq!(capture.pixel(8, 8), red, "the first drawing's surface shows");
    assert_eq!(capture.pixel(40, 8), blue, "and so does the second's");
}
