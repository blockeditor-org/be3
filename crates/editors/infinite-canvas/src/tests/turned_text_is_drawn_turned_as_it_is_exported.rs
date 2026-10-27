use super::copying_as_svg_writes_every_shape_kind::palette_runs;
use super::*;
use block_editor_beui::beui::{Key, Modifiers, Rect, Shape};

const TURN: f32 = 0.5;

fn turned(editor: &BeuiTest<CanvasApp>, written: &str) -> (f32, Rect, Rect) {
    let output = editor.drawn().expect("the canvas has drawn");
    let mut shapes = Vec::new();
    for layer in output.layers.iter() {
        layer.flatten(&mut shapes);
    }
    shapes
        .into_iter()
        .find_map(|shape| match shape {
            Shape::Text {
                origin,
                galley,
                rotation,
                clip,
                ..
            } if galley.text().contains(written) => Some((
                rotation.angle,
                rotation.bounds(Rect::from_min_size(origin, galley.size())),
                clip,
            )),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{written} is drawn"))
}

#[test]
fn turned_text_is_drawn_turned_as_it_is_exported() {
    let mut note = shaped(Uuid::from_u128(1), CanvasShape::Text);
    note.transform = CanvasTransform::new(
        CanvasPoint::new(-120.0, 0.0),
        CanvasPoint::new(160.0, 40.0),
        TURN,
    );
    note.text = "Turned note".into();
    let mut card = card();
    card.id = Uuid::from_u128(2);
    card.transform.center = CanvasPoint::new(140.0, 0.0);
    card.transform.rotation = TURN;
    card.text = "Turned label".into();
    let mut editor = editor(&[note.clone(), card]);
    editor.next_frame();

    for written in ["Turned note", "Turned label"] {
        let (angle, bounds, clip) = turned(&editor, written);
        assert!(
            (angle - TURN).abs() < 1e-5,
            "{written} is drawn at the angle of its shape, not {angle}"
        );
        assert!(
            clip.contains_rect(bounds),
            "{written} is not cut off by the box it was laid out in: {bounds:?} in {clip:?}"
        );
    }
    editor.snapshot("turned_text_is_drawn_turned_as_it_is_exported");

    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    editor.run();
    palette_runs(&mut editor, "copy as svg");
    let copied = editor.take_copied_text();
    let [svg] = copied.as_slice() else {
        panic!("Copy as SVG puts one SVG on the clipboard, put {copied:?}");
    };
    let turns = "transform=\"rotate(28.65 ";
    let texts: Vec<&str> = svg
        .lines()
        .filter(|line| line.starts_with("<text"))
        .collect();
    assert_eq!(texts.len(), 2, "{svg}");
    assert!(
        texts.iter().all(|text| text.contains(turns)),
        "the export turns the text and the label as the canvas does:\n{svg}"
    );

    editor.double_click(format!("infinite-canvas.text.{}", note.id));
    editor.next_frame();
    let (angle, ..) = turned(&editor, "Turned note");
    assert_eq!(angle, 0.0, "text being edited is drawn upright");
}
