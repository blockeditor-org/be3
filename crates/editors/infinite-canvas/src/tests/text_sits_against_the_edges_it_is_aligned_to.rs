use super::*;
use infinite_canvas_types::{CanvasTextAlign, CanvasTextStyle, CanvasTextVerticalAlign};

#[test]
fn text_sits_against_the_edges_it_is_aligned_to() {
    let horizontal = [
        CanvasTextAlign::Left,
        CanvasTextAlign::Center,
        CanvasTextAlign::Right,
    ];
    let vertical = [
        CanvasTextVerticalAlign::Top,
        CanvasTextVerticalAlign::Middle,
        CanvasTextVerticalAlign::Bottom,
    ];
    let mut labels = Vec::new();
    for (row, vertical_alignment) in vertical.into_iter().enumerate() {
        for (column, alignment) in horizontal.into_iter().enumerate() {
            let mut backing = card();
            backing.id = Uuid::from_u128((row * 3 + column + 101) as u128);
            backing.transform = CanvasTransform::new(
                CanvasPoint::new(column as f32 * 150.0 - 150.0, row as f32 * 100.0 - 100.0),
                CanvasPoint::new(140.0, 90.0),
                0.0,
            );
            let mut label = entity(Uuid::from_u128((row * 3 + column + 1) as u128));
            label.transform = backing.transform;
            labels.push(backing);
            label.shape = CanvasShape::Text;
            label.text = "Aligned".into();
            label.text_style = CanvasTextStyle {
                alignment,
                vertical_alignment,
                wrap: true,
                ..CanvasTextStyle::default()
            };
            labels.push(label);
        }
    }
    let mut editor = editor(&labels);
    editor.run();

    editor.snapshot("text_sits_against_the_edges_it_is_aligned_to");
}
