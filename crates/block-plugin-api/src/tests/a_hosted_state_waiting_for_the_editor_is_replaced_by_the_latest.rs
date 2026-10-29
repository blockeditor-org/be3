use super::*;

#[test]
fn a_hosted_state_waiting_for_the_editor_is_replaced_by_the_latest() {
    let hosted = |stacked| {
        DockUpdate::Hosted(DockHostedView {
            view: ViewId::MAIN,
            hosted: Some(DockHosted {
                layout: DockLayout {
                    tree: DockTree {
                        items: Vec::new(),
                        focus: None,
                    },
                    homes: Vec::new(),
                    bars: Vec::new(),
                    outside: Vec::new(),
                    unlocked: Vec::new(),
                },
                tabs: Vec::new(),
                focused: None,
                shown: None,
                visible: Vec::new(),
                fullscreen: None,
                stacked,
                covers: Vec::new(),
            }),
        })
    };
    let close = DockUpdate::Close(DockClosedTab {
        view: ViewId::MAIN,
        tab: DockTabPath::own(1),
    });
    let mut waiting = Vec::new();
    for update in [hosted(false), close.clone(), hosted(true)] {
        DockUpdate::queue(&mut waiting, update);
    }
    assert_eq!(waiting, [close, hosted(true)]);
    round_trips(&waiting);
}
