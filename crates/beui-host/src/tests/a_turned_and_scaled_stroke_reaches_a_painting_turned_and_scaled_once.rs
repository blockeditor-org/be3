use super::*;
use beui_core::painting::{Content, Punches, painting};
use beui_core::painting::{PathCommand, PathStyle};

#[test]
fn a_turned_and_scaled_stroke_reaches_a_painting_turned_and_scaled_once() {
    let context = crate::native::context();
    let output = context.run(|context| {
        let painter = context
            .painter()
            .transformed(vec2(20.0, 10.0), 2.0)
            .rotated(pos2(5.0, 5.0), std::f32::consts::FRAC_PI_2);
        let mut stroke = crate::Stroke::new(1.5);
        stroke.dashes = vec![2.0, 1.0];
        stroke.dash_offset = 0.5;
        painter.stroke_path(
            &crate::Path::new()
                .move_to(pos2(0.0, 0.0))
                .line_to(pos2(10.0, 0.0)),
            &stroke,
            Color32::WHITE,
        );
    });
    let painted = painting(
        output.main(),
        vec2(100.0, 100.0),
        1.0,
        Color32::TRANSPARENT,
        Punches::Shown,
    )
    .expect("it paints");
    let paths: Vec<_> = painted
        .frame
        .primitives
        .iter()
        .filter_map(|primitive| match &primitive.content {
            Content::Path(path) => Some(path),
            _ => None,
        })
        .collect();
    let [path] = paths[..] else {
        panic!("one path is painted, not {}", paths.len());
    };
    let [PathCommand::MoveTo(from), PathCommand::LineTo(to)] = path.commands[..] else {
        panic!("the line is a move and a line: {:?}", path.commands);
    };
    let along = [to[0] - from[0], to[1] - from[1]];
    assert!(
        along[0].abs() < 1e-3 && (along[1].abs() - 20.0).abs() < 1e-3,
        "the line is turned a quarter once and scaled twice once: {along:?}"
    );
    let PathStyle::Stroke(drawn) = &path.style else {
        panic!("the path is stroked");
    };
    assert_eq!(drawn.width, 3.0, "the width is scaled once");
    assert_eq!(drawn.dashes, vec![4.0, 2.0], "the dashes are scaled once");
    assert_eq!(drawn.dash_offset, 1.0, "and their offset");
}
