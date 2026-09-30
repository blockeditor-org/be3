use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn the_layers_panel_selects_reorders_and_hides() {
    let shapes: Vec<CanvasEntity> = (1..=3)
        .map(|index| {
            let mut shape = card();
            shape.id = Uuid::from_u128(index);
            shape.transform.center = CanvasPoint::new(index as f32 * 60.0 - 120.0, 0.0);
            shape
        })
        .collect();
    let [back, middle, front] = [shapes[0].id, shapes[1].id, shapes[2].id];
    let mut editor = open_sized(
        &Canvas::with_entities(shapes),
        false,
        &[],
        Some(Vec2::new(900.0, 1400.0)),
    );
    let row = |editor: &BeuiTest<CanvasApp>, id: Uuid| {
        editor.rect_of(&format!("infinite-canvas.layer.{id}"))
    };
    assert!(
        row(&editor, front).top() < row(&editor, middle).top()
            && row(&editor, middle).top() < row(&editor, back).top(),
        "the frontmost layer is listed first"
    );

    editor.click(&format!("infinite-canvas.layer.{back}"));
    editor.run();
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Rectangle selected"
    );
    editor.click("infinite-canvas.delete");
    editor.run();
    assert_eq!(
        order(&editor),
        vec![middle, front],
        "clicking a layer selected that object"
    );

    let dragged = row(&editor, middle).center();
    let onto = row(&editor, front);
    editor.drag(dragged, onto.center() - Vec2::new(0.0, onto.height() / 4.0));
    editor.run();
    assert_eq!(
        order(&editor),
        vec![front, middle],
        "dropping a layer on the top half of another puts it in front"
    );

    editor.click(&format!("infinite-canvas.layer.{front}.visibility"));
    editor.run();
    let hidden: Vec<_> = entities(&editor)
        .into_iter()
        .filter(|entity| entity.style.hidden)
        .map(|entity| entity.id)
        .collect();
    assert_eq!(hidden, vec![front], "the eye hides just its layer");
    editor.snapshot("the_layers_panel_selects_reorders_and_hides");
}

fn order(editor: &BeuiTest<CanvasApp>) -> Vec<Uuid> {
    entities(editor)
        .into_iter()
        .map(|entity| entity.id)
        .collect()
}
