use super::*;
use crate::reactive::{NodeRef, Text, build, clone, create_memo, create_signal, view};
use crate::unstyled::BackSlide;

const REVEAL: f32 = 200.0;

#[test]
fn the_inspector_swipes_back_as_far_as_its_slider_is_dragged() {
    let (depth, set_depth) = create_signal(1_u32);
    let page = NodeRef::new();
    let page_ref = page.clone();
    let document = build({
        let depth = depth.clone();
        move || {
            let nested = create_memo(clone!(depth -> move || depth.get() > 0));
            view! {
                <BackSlide
                    enabled={nested}
                    on_back={move || set_depth.set(depth.get_untracked() - 1)}
                    behind={|| view! {
                        <Text string="Previous" font_size=14.0 color=Color32::WHITE />
                    }}
                >
                    <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page_ref} />
                </BackSlide>
            }
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(Vec::new());
    let resting = harness.rect(page.get());
    let panel = harness.inspector_panel_rect();
    harness.scroll(panel.center(), vec2(0.0, -REVEAL), Modifiers::NONE);
    harness.frame(Vec::new());
    let track = harness.inspector_rect("inspector.simulation.back");
    assert!(
        track.bottom() < panel.bottom(),
        "the slider is in view: {track:?}"
    );
    let y = track.center().y;
    let along = |fraction: f32| pos2(track.left() + track.width() * fraction, y);

    harness.press_at(along(0.0));
    harness.frame(vec![Event::PointerMoved(along(0.5))]);
    harness.frame(Vec::new());
    let held = harness.rect(page.get());
    assert!(
        held.left() > resting.left(),
        "the page follows the slider: {held:?} from {resting:?}"
    );

    harness.release_at(along(0.5));
    harness.settle();
    assert_eq!(
        harness.rect(page.get()),
        resting,
        "letting go early cancels the swipe"
    );
    assert_eq!(depth.get_untracked(), 1);

    harness.press_at(along(0.0));
    harness.frame(vec![Event::PointerMoved(pos2(track.right() + 40.0, y))]);
    harness.settle();
    assert_eq!(depth.get_untracked(), 0, "the end of the slider goes back");
    harness.release_at(pos2(track.right() + 40.0, y));
    harness.settle();
    assert_eq!(depth.get_untracked(), 0, "back happens once a swipe");
}
