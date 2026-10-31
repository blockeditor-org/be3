use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn the_intrinsic_size_follows_the_first_frame() {
    let mut editor = editor(&[card()]);
    assert_eq!(
        editor.intrinsic_size(),
        Some(Vec2::new(180.0, 120.0)),
        "without an frame an embed shows everything on the canvas"
    );

    let first = frame(7, CanvasPoint::default(), CanvasPoint::new(960.0, 540.0));
    let second = frame(
        8,
        CanvasPoint::new(2000.0, 0.0),
        CanvasPoint::new(300.0, 300.0),
    );
    apply(
        &mut editor,
        InfiniteCanvasOperation::Add {
            entity: Box::new(first),
            pixels: None,
        },
    );
    apply(
        &mut editor,
        InfiniteCanvasOperation::Add {
            entity: Box::new(second),
            pixels: None,
        },
    );
    editor.run();

    assert_eq!(editor.intrinsic_size(), Some(Vec2::new(960.0, 540.0)));

    editor.resize(Vec2::new(480.0, 270.0));
    editor.run();

    assert_eq!(editor.intrinsic_size(), Some(Vec2::new(480.0, 270.0)));
    let sizes: Vec<_> = entities(&editor)
        .into_iter()
        .filter(|entity| entity.is_frame())
        .map(|entity| entity.transform.size)
        .collect();
    assert_eq!(
        sizes,
        [
            CanvasPoint::new(480.0, 270.0),
            CanvasPoint::new(300.0, 300.0)
        ],
        "resizing the embed resizes only the first frame"
    );
}
