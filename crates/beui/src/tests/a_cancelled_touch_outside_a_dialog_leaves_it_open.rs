use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_cancelled_touch_outside_a_dialog_leaves_it_open() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let dismissed = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog
                    open={open}
                    title="Event"
                    width={Size::Custom(300.0)}
                    on_dismiss={move || dismissed.update(|dismissed| *dismissed += 1)}
                >
                    <Text color=Color32::WHITE>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    open.set(true);
    harness.frame(Vec::new());

    let outside = pos2(5.0, VIEWPORT.y - 5.0);
    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::Cancel, outside);
    harness.frame(Vec::new());
    assert_eq!(
        dismissed.get_untracked(),
        0,
        "a touch the system took back, such as the swipe home, is not a tap outside"
    );

    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::End, outside);
    harness.settle();
    assert_eq!(
        dismissed.get_untracked(),
        1,
        "a tap outside dismisses the dialog"
    );
}
