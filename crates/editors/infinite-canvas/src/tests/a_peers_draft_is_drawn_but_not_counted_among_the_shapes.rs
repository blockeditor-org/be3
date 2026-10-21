use super::*;
use crate::presence::CanvasDraft;
use block_editor_beui::beui::{Key, Modifiers, PenPhase, PointerButton};

fn copied_svg(editor: &mut BeuiTest<CanvasApp>) -> String {
    editor.key_press_modifiers(
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
        Key::A,
    );
    editor.run();
    super::copying_as_svg_writes_every_shape_kind::palette_runs(editor, "copy as svg");
    let copied = editor.take_copied_text();
    let [svg] = copied.as_slice() else {
        panic!("Copy as SVG puts one SVG on the clipboard, put {copied:?}");
    };
    svg.clone()
}

#[test]
fn a_peers_draft_is_drawn_but_not_counted_among_the_shapes() {
    let shape = card();
    let mut drawer = vector_editor(&[]);
    let mut watcher = open_sized(
        &vector(vec![shape.clone()]),
        false,
        &[],
        Some(Vec2::new(900.0, 1400.0)),
    );
    drawer.presence_visible(true);
    drawer.run();
    let alone = copied_svg(&mut watcher);
    let mut held = BTreeMap::new();

    drawer.click("infinite-canvas.dock.tool.Rectangle");
    drawer.run();
    let from = drawer.rect_of("infinite-canvas.canvas").center() + Vec2::new(100.0, 100.0);
    drawer.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    drawer.next_frame();
    drawer.pen(PenPhase::Move, from + Vec2::new(300.0, 200.0), 0.5);
    drawer.run();
    forward_presence(&mut drawer, &mut watcher, &mut held);
    let drafted: Vec<CanvasDraft> = held
        .iter()
        .filter(|(kind, _)| CanvasDraft::holds(**kind))
        .map(|(_, value)| serde_json::from_slice(value).unwrap())
        .collect();
    let [CanvasDraft::Shape(drawn)] = &drafted[..] else {
        panic!("a rectangle being dragged is one draft, drafted {drafted:?}");
    };
    assert_eq!(drawn.shape, CanvasShape::Rectangle);
    let document = watcher.document();
    assert!(
        document
            .find_test_id(&format!("infinite-canvas.entity.{}", drawn.id))
            .is_some(),
        "the peer draws the rectangle being drawn"
    );
    assert!(
        document
            .find_test_id(&format!("infinite-canvas.layer.{}", drawn.id))
            .is_none()
            && document
                .find_test_id(&format!("infinite-canvas.layer.{}", shape.id))
                .is_some(),
        "the layers panel lists only what is stored"
    );
    assert_eq!(
        watcher.intrinsic_size(),
        Some(Vec2::new(180.0, 120.0)),
        "the canvas is sized to what is stored"
    );
    let svg = copied_svg(&mut watcher);
    assert_eq!(
        watcher.label("infinite-canvas.selection"),
        "Rectangle selected",
        "selecting everything leaves the draft out"
    );
    assert_eq!(svg, alone, "the export leaves the draft out");
    assert_eq!(entities(&watcher), vec![shape]);
}
