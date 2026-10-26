use beui::WindowSize;
use beui::testing::{Showing, Stepped};
use beui::{Context, FreetypeFonts};

use super::*;

const SIZE: Vec2 = vec2(1100.0, 720.0);

fn paint(stepped: &mut Stepped, name: &str) {
    let output = stepped.frame(Vec::new());
    let painted = beui::painting::painting(
        output.main(),
        SIZE,
        1.0,
        output.main().window.appearance.clear_color,
        beui::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(name, &snapshot, "./scripts/verify");
}

fn shown(content: impl FnOnce() -> NodeId + 'static) -> Stepped {
    let document = build(move || {
        view! {
            <beui::styled::Root>{content()}</beui::styled::Root>
        }
    });
    let mut stepped = Stepped::new(
        Context::new(FreetypeFonts::default()),
        Showing(document),
        WindowSize {
            size: SIZE,
            scale: 1.0,
        },
        None,
    );
    stepped.frame(Vec::new());
    stepped.frame(Vec::new());
    stepped
}

#[test]
fn a_conflicted_block_offers_both_versions() {
    let block = Uuid::from_u128(7);
    let mut notice = shown(move || {
        view! {
            <Layers>
                <Frame fill=ColorRole::Background />
                <ConflictNotice block count=2 />
            </Layers>
        }
    });
    assert!(
        notice
            .document()
            .find_test_id(&format!("conflict.{block}.compare"))
            .is_some()
    );
    paint(
        &mut notice,
        "a_conflicted_block_shows_a_notice_on_the_block",
    );

    let mut dialog = shown(move || {
        let comparison = create_memo(move || {
            Some(crate::be::conflicts::Comparison {
                block,
                ours: Some(Err(
                    "This version is still loading from this device.".to_owned()
                )),
                theirs: Some(Err("This version is no longer available.".to_owned())),
                error: None,
            })
        });
        view! {
            <ConflictDialog comparison />
        }
    });
    for side in ["conflict.compare.ours", "conflict.compare.theirs"] {
        assert!(dialog.document().find_test_id(side).is_some());
    }
    paint(
        &mut dialog,
        "a_conflicted_block_shows_both_versions_side_by_side",
    );
}
