use super::*;

fn line(style: PathStyle) -> PathShape {
    PathShape {
        commands: vec![
            PathCommand::MoveTo([0.0, 0.0]),
            PathCommand::LineTo([10.0, 0.0]),
            PathCommand::LineTo([10.0, 10.0]),
        ],
        style,
        color: [255; 4],
        seamed: false,
    }
}

fn stroke(join: Join, cap: Cap) -> PathStyle {
    PathStyle::Stroke(PathStroke {
        width: 2.0,
        join,
        miter_limit: 4.0,
        cap,
        dashes: vec![3.0, 1.0],
        dash_offset: 1.0,
    })
}

#[test]
fn a_path_s_bounds_reach_its_stroke_and_mapping_moves_and_scales_it() {
    let filled = line(PathStyle::Fill { even_odd: false });
    assert_eq!(filled.bounds(), [0.0, 0.0, 10.0, 10.0]);
    // Half the width, times the miter limit at a miter join...
    let mitred = line(stroke(Join::Miter, Cap::Butt));
    assert_eq!(mitred.bounds(), [-4.0, -4.0, 14.0, 14.0]);
    // ...or the square cap's diagonal.
    let square = line(stroke(Join::Round, Cap::Square)).bounds();
    let reach = std::f32::consts::SQRT_2;
    assert_eq!(square, [-reach, -reach, 10.0 + reach, 10.0 + reach]);
    assert_eq!(
        line(stroke(Join::Bevel, Cap::Round)).bounds(),
        [-1.0, -1.0, 11.0, 11.0]
    );

    let empty = PathShape {
        commands: vec![PathCommand::Close],
        ..filled.clone()
    };
    let [left, top, right, bottom] = empty.bounds();
    assert!(left >= right && top >= bottom, "an empty path covers nothing");

    let mapped = mitred.mapped(|[x, y]| [x * 2.0 + 1.0, y * 2.0], 2.0);
    assert_eq!(
        mapped.commands,
        vec![
            PathCommand::MoveTo([1.0, 0.0]),
            PathCommand::LineTo([21.0, 0.0]),
            PathCommand::LineTo([21.0, 20.0]),
        ]
    );
    let PathStyle::Stroke(scaled) = &mapped.style else {
        panic!("still stroked");
    };
    assert_eq!(
        (scaled.width, scaled.dashes.clone(), scaled.dash_offset),
        (4.0, vec![6.0, 2.0], 2.0)
    );
}
