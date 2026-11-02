use super::*;

#[test]
fn a_preview_keeps_the_glyphs_it_shows_where_they_were() {
    let glyph = Primitive {
        clip: [0.0, 0.0, 176.0, 99.0],
        fade: Fade::NONE,
        rounds: Vec::new(),
        content: Content::Glyph(Glyph {
            rect: [78.0, 34.0, 85.0, 43.0],
            texture: 1,
            color: [255; 4],
            turn: Turn::NONE,
        }),
    };
    let child = Painting {
        frame: Frame {
            size: [176, 99],
            pixels_per_point: 1.0,
            background: [0; 4],
            primitives: vec![glyph],
        },
        textures: BTreeMap::from([(
            1,
            Pixels {
                size: [1, 1],
                rgba: vec![[255; 4]],
            },
        )]),
    };
    let key = preview_key(3);
    let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
        pos: [x, y],
        uv: [u, v],
        color: [255; 4],
    };
    let parent = Painting {
        frame: Frame {
            size: [198, 668],
            pixels_per_point: 1.0,
            background: [0; 4],
            primitives: vec![Primitive {
                clip: [17.0, 16.0, 193.0, 115.0],
                fade: Fade::NONE,
                rounds: Vec::new(),
                content: Content::Mesh(vec![
                    Triangle {
                        texture: key,
                        corners: [
                            corner(17.0, 16.0, 0.0, 0.0),
                            corner(193.0, 16.0, 1.0, 0.0),
                            corner(193.0, 115.0, 1.0, 1.0),
                        ],
                    },
                    Triangle {
                        texture: key,
                        corners: [
                            corner(17.0, 16.0, 0.0, 0.0),
                            corner(193.0, 115.0, 1.0, 1.0),
                            corner(17.0, 115.0, 0.0, 1.0),
                        ],
                    },
                ]),
            }],
        },
        textures: BTreeMap::new(),
    };
    let shown = parent.with_previews(|_| Some(child.clone()));
    let Content::Glyph(placed) = &shown.frame.primitives[0].content else {
        panic!("the glyph is placed");
    };
    assert_eq!(placed.rect, [95.0, 50.0, 102.0, 59.0]);
}
