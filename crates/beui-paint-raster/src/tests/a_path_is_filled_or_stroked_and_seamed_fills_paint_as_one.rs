use super::*;
use beui_paint_format::{Cap, Join, PathCommand, PathShape, PathStroke, PathStyle};

fn polygon(points: &[[f32; 2]]) -> Vec<PathCommand> {
    let mut commands = vec![PathCommand::MoveTo(points[0])];
    commands.extend(points[1..].iter().map(|at| PathCommand::LineTo(*at)));
    commands.push(PathCommand::Close);
    commands
}

fn drawn(paths: Vec<PathShape>) -> image::RgbaImage {
    let primitives = paths
        .into_iter()
        .map(|path| Primitive {
            clip: [0.0, 0.0, 16.0, 16.0],
            fade: Fade::NONE,
            rounds: Vec::new(),
            content: Content::Path(path),
        })
        .collect();
    let frame = Frame {
        size: [16, 16],
        pixels_per_point: 1.0,
        background: [0, 0, 0, 255],
        primitives,
    };
    crate::render(&Snapshot::of(frame, BTreeMap::new()), 0).unwrap()
}

fn fill(commands: Vec<PathCommand>, seamed: bool) -> PathShape {
    PathShape {
        commands,
        style: PathStyle::Fill { even_odd: false },
        color: [255, 255, 255, 128],
        seamed,
    }
}

#[test]
fn a_path_is_filled_or_stroked_and_seamed_fills_paint_as_one() {
    let square = drawn(vec![fill(
        polygon(&[[2.0, 2.0], [14.0, 2.0], [14.0, 14.0], [2.0, 14.0]]),
        false,
    )]);
    assert_eq!(square.get_pixel(8, 8).0, [128, 128, 128, 255]);
    assert_eq!(square.get_pixel(1, 8).0, [0, 0, 0, 255]);

    let line = drawn(vec![PathShape {
        commands: vec![PathCommand::MoveTo([2.0, 8.0]), PathCommand::LineTo([14.0, 8.0])],
        style: PathStyle::Stroke(PathStroke {
            width: 2.0,
            join: Join::Miter,
            miter_limit: 4.0,
            cap: Cap::Butt,
            dashes: vec![4.0, 4.0],
            dash_offset: 0.0,
        }),
        color: [255, 255, 255, 255],
        seamed: false,
    }]);
    assert_eq!(line.get_pixel(3, 8).0, [255, 255, 255, 255], "a dash");
    assert_eq!(line.get_pixel(7, 8).0, [0, 0, 0, 255], "a gap");
    assert_eq!(line.get_pixel(3, 5).0, [0, 0, 0, 255], "off the line");

    // Two halves meeting on a slanted seam: apart, each antialiases the edge they share and
    // the seam shows; seamed, they fill as one, as the renderer fills them.
    let halves = |seamed: bool| {
        drawn(vec![
            fill(polygon(&[[2.0, 2.0], [7.0, 2.0], [9.0, 14.0], [2.0, 14.0]]), seamed),
            fill(polygon(&[[7.0, 2.0], [14.0, 2.0], [14.0, 14.0], [9.0, 14.0]]), seamed),
        ])
    };
    let seam = |image: &image::RgbaImage| (2..14).any(|y| image.get_pixel(8, y).0 != [128, 128, 128, 255]);
    assert!(seam(&halves(false)), "the halves apart show their seam");
    assert!(!seam(&halves(true)), "seamed halves paint as one fill");
}
