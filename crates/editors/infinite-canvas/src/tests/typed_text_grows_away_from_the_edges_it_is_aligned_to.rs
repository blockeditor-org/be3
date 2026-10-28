use super::*;
use block_editor_beui::beui::{Key, Modifiers};
use infinite_canvas_types::{CanvasTextAlign, CanvasTextStyle, CanvasTextVerticalAlign};

fn label(
    id: u128,
    center: CanvasPoint,
    alignment: CanvasTextAlign,
    vertical_alignment: CanvasTextVerticalAlign,
) -> CanvasEntity {
    let mut label = entity(Uuid::from_u128(id));
    label.transform = CanvasTransform::new(center, CanvasPoint::new(60.0, 60.0), 0.0);
    label.shape = CanvasShape::Text;
    label.text = "Hi".into();
    label.text_style = CanvasTextStyle {
        alignment,
        vertical_alignment,
        ..CanvasTextStyle::default()
    };
    label
}

#[test]
fn typed_text_grows_away_from_the_edges_it_is_aligned_to() {
    let labels = [
        label(
            1,
            CanvasPoint::new(-150.0, -100.0),
            CanvasTextAlign::Left,
            CanvasTextVerticalAlign::Top,
        ),
        label(
            2,
            CanvasPoint::new(0.0, 0.0),
            CanvasTextAlign::Center,
            CanvasTextVerticalAlign::Middle,
        ),
        label(
            3,
            CanvasPoint::new(150.0, 100.0),
            CanvasTextAlign::Right,
            CanvasTextVerticalAlign::Bottom,
        ),
    ];
    let mut editor = editor(&labels);

    for label in &labels {
        editor.click(format!("infinite-canvas.entity.{}", label.id));
        editor.key(Key::Enter, Modifiers::NONE);
        editor.type_text("Hi there, a longer line");
        editor.key(Key::Escape, Modifiers::NONE);
    }

    let held = entities(&editor);
    let edges = |entity: &CanvasEntity| {
        let CanvasTransform { center, size, .. } = entity.transform;
        (
            center.x - size.x / 2.0,
            center.x + size.x / 2.0,
            center.y - size.y / 2.0,
            center.y + size.y / 2.0,
        )
    };
    for (before, after) in labels.iter().zip(&held) {
        assert!(
            after.transform.size.x > 120.0,
            "the text grew: {:?}",
            after.transform.size
        );
        assert_ne!(after.transform.size.y, 60.0, "the box fits the text");
        let (left, right, top, bottom) = edges(before);
        let (new_left, new_right, new_top, new_bottom) = edges(after);
        let text_style = after.text_style;
        match text_style.alignment {
            CanvasTextAlign::Left => assert!((new_left - left).abs() < 0.01),
            CanvasTextAlign::Center => {
                assert!((after.transform.center.x - before.transform.center.x).abs() < 0.01)
            }
            CanvasTextAlign::Right => assert!((new_right - right).abs() < 0.01),
        }
        match text_style.vertical_alignment {
            CanvasTextVerticalAlign::Top => assert!((new_top - top).abs() < 0.01),
            CanvasTextVerticalAlign::Middle => {
                assert!((after.transform.center.y - before.transform.center.y).abs() < 0.01)
            }
            CanvasTextVerticalAlign::Bottom => assert!((new_bottom - bottom).abs() < 0.01),
        }
    }

    editor.snapshot("typed_text_grows_away_from_the_edges_it_is_aligned_to");
}
