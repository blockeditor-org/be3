use super::*;
use crate::reactive::*;
use crate::unstyled::BackSlide;
use crate::{PreferenceLayer, PreferenceSource};
use beui::BackGesture;

#[test]
fn instant_back_slides_complete_without_waiting_for_frames() {
    let _root = RootScope::new();
    let backs = create_signal(0);
    let document = build_node(move || {
        view! {
            <BackSlide on_back={move || backs.update(|n| *n += 1)}>
                <Frame height=20.0 />
            </BackSlide>
        }
    });
    let mut harness = Harness::new(document);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 1);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
}
