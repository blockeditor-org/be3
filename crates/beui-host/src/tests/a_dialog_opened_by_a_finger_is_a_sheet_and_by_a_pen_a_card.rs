use super::*;
use crate::NodeId;
use crate::primitives::{List, Text};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::DialogLayout;
use beui::input::{Pen, PenPhase};

#[test]
fn a_dialog_opened_by_a_finger_is_a_sheet_and_by_a_pen_a_card() {
    let _root = crate::reactive::RootScope::new();
    let opener = NodeRef::new();
    let open = create_signal(false);
    let sheet = create_signal(None::<bool>);
    let document = build_node(move || {
        view! {
            <List spacing=8.0>
                <Button @node_ref=&opener label="Open" on_click={move || open.set(true)} />
                <Dialog open={open} title="Rename" on_dismiss={move || open.set(false)}>
                    <Probe sheet />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(1200.0, 800.0));
    harness.frame(Vec::new());
    let at = harness.center(opener.get());

    harness.tap(at);
    harness.frame(Vec::new());
    assert!(open.get_untracked());
    assert_eq!(sheet.get_untracked(), Some(true), "a finger opens a sheet");

    open.set(false);
    harness.settle();
    let pen = |phase| Input::Pen {
        phase,
        pos: at,
        pen: Pen {
            pressure: 0.5,
            tilt: Vec2::ZERO,
            eraser: false,
        },
    };
    harness.frame(vec![pen(PenPhase::Move)]);
    harness.frame(vec![pen(PenPhase::Down(PointerButton::Primary))]);
    harness.frame(vec![pen(PenPhase::Up(PointerButton::Primary))]);
    harness.frame(Vec::new());
    assert!(open.get_untracked());
    assert_eq!(
        sheet.get_untracked(),
        Some(false),
        "a pen points like a mouse, so it opens the card, as it drops a menu down"
    );
}

#[component]
fn Probe(sheet: Signal<Option<bool>>) -> NodeId {
    let layout = use_context::<DialogLayout>().expect("a dialog provides its layout");
    create_effect(move || sheet.set(Some(layout.sheet.get())));
    view! {
        <Text color=Color32::WHITE>"Body"</Text>
    }
}
