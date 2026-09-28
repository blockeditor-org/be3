use super::*;
use crate::reactive::{Text, build, create_signal, view, with_reactive_scope};
use crate::styled::Dialog;

#[test]
fn a_cancelled_touch_outside_a_dialog_leaves_it_open() {
    let (open, set_open) = create_signal(false);
    let dismissed = Rc::new(Cell::new(0));
    let reports = dismissed.clone();
    let document = build({
        let open = open.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog
                        open={open}
                        title="Event"
                        width=300.0
                        on_dismiss={move || reports.set(reports.get() + 1)}
                    >
                        <Text string="Body" font_size=14.0 color=Color32::WHITE />
                    </Dialog>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());

    let outside = pos2(5.0, VIEWPORT.y - 5.0);
    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::Cancel, outside);
    harness.frame(Vec::new());
    assert_eq!(
        dismissed.get(),
        0,
        "a touch the system took back, such as the swipe home, is not a tap outside"
    );

    harness.touch(TouchPhase::Start, outside);
    harness.touch(TouchPhase::End, outside);
    harness.frame(Vec::new());
    assert_eq!(dismissed.get(), 1, "a tap outside dismisses the dialog");
}
