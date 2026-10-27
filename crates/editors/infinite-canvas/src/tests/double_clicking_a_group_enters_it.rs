use super::*;
use block_editor_beui::beui::{Key, Modifiers, Pos2};

#[test]
fn double_clicking_a_group_enters_it() {
    let group = Some(Uuid::from_u128(9));
    let mut left = card();
    left.style.stroke_width = 0.0;
    left.transform = CanvasTransform::new(
        CanvasPoint::new(-80.0, 0.0),
        CanvasPoint::new(100.0, 60.0),
        0.0,
    );
    left.group_id = group;
    let mut right = left.clone();
    right.id = Uuid::from_u128(2);
    right.transform.center = CanvasPoint::new(80.0, 0.0);
    let mut outside = left.clone();
    outside.id = Uuid::from_u128(3);
    outside.group_id = None;
    outside.transform.center = CanvasPoint::new(0.0, 140.0);
    let mut editor = vector_editor(&[left.clone(), right.clone(), outside.clone()]);
    let anchor = editor.rect_of(&format!("infinite-canvas.entity.{}", left.id));
    let scale = anchor.width() / 104.0;
    let at = |x: f32, y: f32| -> Pos2 { anchor.center() + Vec2::new(x + 80.0, y) * scale };

    editor.click(at(80.0, 0.0));
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Group · 2 objects",
        "a click selects the whole group"
    );

    editor.double_click(at(80.0, 0.0));
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Rectangle selected",
        "a double click selects the one member"
    );
    editor.snapshot("double_clicking_a_group_enters_it");

    editor.click(at(-80.0, 0.0));
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Rectangle selected",
        "inside the group, a click selects another member alone"
    );
    editor.advance(std::time::Duration::from_secs(1));
    editor.run();
    editor.drag(at(-80.0, 0.0), at(-80.0, -40.0));
    let held = entities(&editor);
    let moved = |id: Uuid| {
        held.iter()
            .find(|entity| entity.id == id)
            .expect("the member is on the canvas")
            .transform
            .center
    };
    assert!(
        (moved(left.id).y + 40.0).abs() < 0.5 && moved(right.id).y.abs() < 0.5,
        "dragging a member moves only it"
    );

    editor.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Group · 2 objects",
        "Escape leaves the group and selects all of it"
    );

    editor.double_click(at(80.0, 0.0));
    editor.click(at(0.0, 140.0));
    editor.click(at(80.0, 0.0));
    assert_eq!(
        editor.label("infinite-canvas.selection"),
        "Group · 2 objects",
        "clicking outside the group leaves it"
    );
}
