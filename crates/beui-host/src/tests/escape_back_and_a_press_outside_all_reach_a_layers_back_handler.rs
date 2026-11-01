use super::*;
use crate::Dismiss;
use crate::primitives::*;
use crate::reactive::*;
use beui::BackGesture;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn escape_back_and_a_press_outside_all_reach_a_layers_back_handler() {
    let _root = crate::reactive::RootScope::new();
    let backs = create_signal(0);
    let dismissed = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Frame>
                <Overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open=true
                    on_dismiss={move |reason: Dismiss| dismissed.update(|dismissed| dismissed.push(reason))}
                >
                    <BackTo
                        on_gesture={move |gesture: BackGesture| if gesture == BackGesture::Invoked { backs.update(|backs| *backs += 1) }}
                    >
                        <Frame width=40.0 height=30.0 />
                    </BackTo>
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        backs.get_untracked(),
        1,
        "Escape goes to the layer's back handler"
    );
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 2, "and so does back");

    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(Vec::new());
    assert_eq!(backs.get_untracked(), 3, "and so does a press outside");
    assert!(
        dismissed.with_untracked(|dismissed| dismissed.is_empty()),
        "none of them skip the handler for the layer's own on_dismiss"
    );
}
