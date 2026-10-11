use super::*;

fn square(side: i32, argb: [u8; 4]) -> Pixmap {
    Pixmap {
        width: side,
        height: side,
        argb: argb.repeat((side * side) as usize),
    }
}

#[test]
fn the_pixmap_nearest_above_the_size_is_turned_to_rgba() {
    let pixmaps = [
        square(16, [0xff, 1, 1, 1]),
        square(64, [0xff, 2, 2, 2]),
        square(32, [0x80, 0x10, 0x20, 0x30]),
        Pixmap {
            width: 8,
            height: 8,
            argb: vec![0; 3],
        },
    ];
    let image = pixmap_image(&pixmaps, 32).expect("a pixmap fits");
    assert_eq!((image.width, image.height), (32, 32));
    assert_eq!(
        &image.rgba[..4],
        &[0x10, 0x20, 0x30, 0x80],
        "network-order ARGB becomes RGBA"
    );

    let image = pixmap_image(&pixmaps, 24).expect("a pixmap fits");
    assert_eq!((image.width, image.height), (24, 24));
    assert_eq!(
        &image.rgba[..4],
        &[0x10, 0x20, 0x30, 0x80],
        "the smallest at least as big is shrunk to the size"
    );

    let image = pixmap_image(&pixmaps, 128).expect("a pixmap fits");
    assert_eq!((image.width, image.height), (64, 64), "else the biggest");

    let image = pixmap_image(&[square(64, [0xff, 9, 9, 9])], 48).expect("a pixmap fits");
    assert_eq!(
        (image.width, image.height),
        (48, 48),
        "one too big is shrunk to the size"
    );
    assert!(image.is_whole());

    assert_eq!(
        pixmap_image(&pixmaps[3..], 24),
        None,
        "a short pixmap is no picture"
    );
}
