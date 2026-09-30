use super::*;
use block_editor_beui::be_block::canvas::{CanvasColor, CanvasTextStyle};

#[test]
fn the_canvas_paints_the_entities_it_holds() {
    let mut rectangle = entity(Uuid::from_u128(1));
    rectangle.transform = CanvasTransform::new(
        CanvasPoint::new(-80.0, -40.0),
        CanvasPoint::new(140.0, 90.0),
        0.0,
    );
    rectangle.style.fill = Some(CanvasColor::Rgba {
        red: 40,
        green: 90,
        blue: 160,
        alpha: 255,
    });
    rectangle.style.corner_radius = 8.0;

    let mut turned = entity(Uuid::from_u128(2));
    turned.transform = CanvasTransform::new(
        CanvasPoint::new(90.0, -40.0),
        CanvasPoint::new(110.0, 70.0),
        0.5,
    );

    let mut label = entity(Uuid::from_u128(3));
    label.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, 70.0),
        CanvasPoint::new(220.0, 40.0),
        0.0,
    );
    label.kind = CanvasEntityKind::Text {
        text: "Canvas".to_owned(),
        text_style: CanvasTextStyle::default(),
        placeholder: "Text".to_owned(),
    };

    let mut arrow = entity(Uuid::from_u128(4));
    arrow.transform = CanvasTransform::new(
        CanvasPoint::new(-60.0, 130.0),
        CanvasPoint::new(180.0, 1.0),
        0.0,
    );
    arrow.kind = CanvasEntityKind::Line;
    arrow.style.arrow_end = true;

    let mut editor = editor(&[rectangle, turned, label, arrow]);
    editor.run();

    editor.snapshot("the_canvas_paints_the_entities_it_holds");
}
