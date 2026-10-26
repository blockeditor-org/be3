use super::*;

#[test]
fn a_painting_keeps_its_texture_pixels_through_encoding() {
    let painting = Painting {
        frame: Frame {
            size: [2, 1],
            pixels_per_point: 1.0,
            background: [0, 0, 0, 255],
            primitives: Vec::new(),
        },
        textures: BTreeMap::from([(
            7,
            Pixels {
                size: [2, 1],
                rgba: vec![[255, 128, 0, 255], [1, 2, 3, 4]],
            },
        )]),
    };

    assert_eq!(Painting::decode(&painting.encode()).unwrap(), painting);
}
