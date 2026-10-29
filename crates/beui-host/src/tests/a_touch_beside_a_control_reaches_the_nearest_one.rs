use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_touch_beside_a_control_reaches_the_nearest_one() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal::<Vec<&'static str>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Frame padding=40.0>
                <List direction=Direction::Horizontal spacing=10.0>
                    <Interactive on_click={move || clicks.update(|clicks| clicks.push("left"))}>
                        <Frame width=30.0 height=30.0 />
                    </Interactive>
                    <Interactive on_click={move || clicks.update(|clicks| clicks.push("right"))}>
                        <Frame width=30.0 height=30.0 />
                    </Interactive>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tap = |harness: &mut Harness, pos: Pos2| {
        harness.touch(TouchPhase::Start, pos);
        harness.touch(TouchPhase::End, pos);
    };

    tap(&mut harness, pos2(32.0, 55.0));
    tap(&mut harness, pos2(73.0, 55.0));
    tap(&mut harness, pos2(77.0, 55.0));
    tap(&mut harness, pos2(95.0, 82.0));
    assert_eq!(
        clicks.get_untracked(),
        vec!["left", "left", "right", "right"],
        "a touch beside a control goes to the nearest one, and neighbours split the gap"
    );

    clicks.update(|clicks| clicks.clear());
    tap(&mut harness, pos2(10.0, 55.0));
    harness.click(pos2(32.0, 55.0));
    assert!(
        clicks.with_untracked(|clicks| clicks.is_empty()),
        "a touch out of reach, and a mouse beside a control, press nothing"
    );
}
