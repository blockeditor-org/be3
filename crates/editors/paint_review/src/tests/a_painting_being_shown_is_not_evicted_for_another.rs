use block_editor_beui::beui::Image;

use crate::render::{Painted, Paintings};

const SIDE: u32 = 2048;
const FRAMES: usize = 2;

#[test]
fn a_painting_being_shown_is_not_evicted_for_another() {
    let mut paintings = Paintings::default();
    paintings.keep(vec!["shown".to_owned()]);

    hold(&mut paintings, "shown", 30);
    hold(&mut paintings, "other", 200);

    assert!(paintings.rendered("shown", 0).is_some());
    assert!(paintings.rendered("other", 0).is_none());
}

fn hold(paintings: &mut Paintings, hash: &str, shade: u8) {
    for frame in 0..FRAMES {
        assert!(
            paintings
                .computed(hash, frame, FRAMES, || Ok(large(shade)))
                .is_ok()
        );
    }
}

fn large(shade: u8) -> Painted {
    let pixels = vec![shade; SIDE as usize * SIDE as usize * 4];
    Painted {
        image: Image::from_rgba(SIDE, SIDE, pixels),
        description: format!("a painting too large to hold beside another, shaded {shade}"),
    }
}
