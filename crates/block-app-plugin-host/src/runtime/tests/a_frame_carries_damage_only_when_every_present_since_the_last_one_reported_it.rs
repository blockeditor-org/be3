use super::*;
use block_plugin_api::SurfaceRect;

fn presented(sequence: u64, x: u32) -> PresentedFrame {
    PresentedFrame {
        sequence,
        damage: vec![rect(x)],
    }
}

fn rect(x: u32) -> ScreenDamage {
    ScreenDamage {
        screen: ScreenId(1),
        rect: SurfaceRect {
            x,
            y: 0,
            width: 1,
            height: 1,
        },
    }
}

#[test]
fn a_frame_carries_damage_only_when_every_present_since_the_last_one_reported_it() {
    let mut presents = Presents::default();
    presents.report(presented(1, 10));
    assert_eq!(presents.damage_through(1), Some(vec![rect(10)]));

    presents.report(presented(2, 20));
    presents.report(presented(3, 30));
    assert_eq!(
        presents.damage_through(3),
        Some(vec![rect(20), rect(30)]),
        "two presents between frames damage both",
    );

    presents.report(presented(5, 50));
    assert_eq!(
        presents.damage_through(5),
        None,
        "a present that reported nothing leaves the whole surface damaged",
    );

    assert_eq!(
        presents.damage_through(6),
        None,
        "a frame whose report has not arrived yet damages the whole surface",
    );
    presents.report(presented(6, 60));
    presents.report(presented(7, 70));
    assert_eq!(presents.damage_through(7), Some(vec![rect(70)]));
}
