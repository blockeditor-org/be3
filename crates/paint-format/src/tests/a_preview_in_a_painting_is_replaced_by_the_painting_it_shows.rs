use super::*;

fn square(rect: [f32; 4], color: [u8; 4]) -> Primitive {
    Primitive {
        clip: [-100.0, -100.0, 100.0, 100.0],
        fade: Fade::NONE,
        rounds: Vec::new(),
        content: Content::RoundedRect(RoundedRect {
            rect,
            corner_radius: 0.0,
            stroke_width: 0.0,
            color,
            turn: Turn::NONE,
        }),
    }
}

fn shown(image: u64, rect: [f32; 4]) -> Primitive {
    let key = preview_key(image);
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color: [255; 4],
    };
    let [left, top, right, bottom] = rect;
    Primitive {
        clip: [0.0, 0.0, 50.0, 50.0],
        fade: Fade::NONE,
        rounds: Vec::new(),
        content: Content::Mesh(vec![
            Triangle {
                texture: key,
                corners: [
                    corner(left, top, 0.0, 0.0),
                    corner(right, top, 1.0, 0.0),
                    corner(right, bottom, 1.0, 1.0),
                ],
            },
            Triangle {
                texture: key,
                corners: [
                    corner(left, top, 0.0, 0.0),
                    corner(right, bottom, 1.0, 1.0),
                    corner(left, bottom, 0.0, 1.0),
                ],
            },
        ]),
    }
}

fn painting(primitives: Vec<Primitive>) -> Painting {
    Painting {
        frame: Frame {
            size: [10, 10],
            pixels_per_point: 1.0,
            background: [0; 4],
            primitives,
        },
        textures: BTreeMap::new(),
    }
}

#[test]
fn a_preview_in_a_painting_is_replaced_by_the_painting_it_shows() {
    let parent = painting(vec![
        square([0.0, 0.0, 5.0, 5.0], [1, 2, 3, 255]),
        shown(7, [20.0, 20.0, 40.0, 40.0]),
        shown(8, [0.0, 0.0, 10.0, 10.0]),
    ]);
    assert_eq!(parent.previews(), vec![7, 8]);
    let child = painting(vec![square([0.0, 0.0, 5.0, 10.0], [9, 9, 9, 255])]);
    let replaced = parent.with_previews(|image| (image == 7).then(|| child.clone()));
    assert_eq!(
        replaced.frame.primitives.len(),
        2,
        "the preview with a painting is replaced, and the one without is left out"
    );
    let Content::RoundedRect(placed) = &replaced.frame.primitives[1].content else {
        panic!("the preview's own square is placed in its stead");
    };
    assert_eq!(
        placed.rect,
        [20.0, 20.0, 30.0, 40.0],
        "the preview's painting is scaled into the place the parent showed it"
    );
    assert!(replaced.previews().is_empty());
}
