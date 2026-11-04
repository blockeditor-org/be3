use super::*;
use std::time::Duration;

#[test]
fn a_layer_carried_up_the_panel_parts_the_layers_it_passes() {
    let kinds = [
        CanvasShape::Rectangle,
        CanvasShape::Ellipse,
        CanvasShape::Diamond,
    ];
    let shapes: Vec<CanvasEntity> = (1..=3)
        .map(|index| {
            let mut shape = card();
            shape.id = Uuid::from_u128(index);
            shape.shape = kinds[index as usize - 1];
            shape.transform.center = CanvasPoint::new(index as f32 * 60.0 - 120.0, 0.0);
            shape
        })
        .collect();
    let [back, middle, front] = [shapes[0].id, shapes[1].id, shapes[2].id];
    let mut editor = open_sized(&vector(shapes), false, &[], Some(Vec2::new(900.0, 1400.0)));
    let row = |editor: &BeuiTest<CanvasApp>, id: Uuid| {
        editor.rect_of(&format!("infinite-canvas.layer.{id}"))
    };

    let grab = row(&editor, back).center();
    let past = row(&editor, middle).center() - Vec2::new(0.0, 6.0);
    editor.down(grab, PointerButton::Primary);
    editor.next_frame();
    editor.hover(grab - Vec2::new(0.0, 6.0));
    editor.next_frame();
    editor.hover(past);
    editor.run();
    editor.record();
    for _ in 0..3 {
        editor.advance(Duration::from_millis(50));
        editor.record();
    }
    assert_eq!(
        order(&editor),
        vec![back, middle, front],
        "nothing moves until the layer is dropped"
    );

    editor.up(past, PointerButton::Primary);
    editor.run();
    editor.record();
    editor.settle();
    assert_eq!(
        order(&editor),
        vec![middle, back, front],
        "the layer goes in front of the one it was carried past"
    );
    editor.snapshot("a_layer_carried_up_the_panel_parts_the_layers_it_passes");
}

fn order(editor: &BeuiTest<CanvasApp>) -> Vec<Uuid> {
    entities(editor)
        .into_iter()
        .map(|entity| entity.id)
        .collect()
}
