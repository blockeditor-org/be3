use super::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use crate::{PreferenceLayer, PreferenceSource};

#[test]
fn instant_scrolling_follows_the_finger_without_flinging() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(400)}>|_: usize|
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Spacer />
                        </Frame>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;

    harness.touch(TouchPhase::Start, pos2(x, 280.0));
    for y in [250.0, 220.0, 190.0, 160.0] {
        harness.touch(TouchPhase::Move, pos2(x, y));
    }
    harness.touch(TouchPhase::End, pos2(x, 160.0));
    let released = harness.document().scroll_offset(scroll);

    assert!(released > 0.0, "held gestures still scroll immediately");
    harness.advance(Duration::from_secs(2));
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), released);
    assert!(
        !harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
}
