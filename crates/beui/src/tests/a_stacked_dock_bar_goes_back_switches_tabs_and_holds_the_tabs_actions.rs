use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout, TabId, dock_menu};
use crate::reactive::*;

const FIRST: u64 = 1;

fn laid_out(harness: &Harness, test_id: &str) -> bool {
    harness
        .document()
        .find_test_id(test_id)
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

fn count(harness: &Harness) -> String {
    let document = harness.document();
    let mut pending = vec![harness.find("dock.switch")];
    while let Some(node) = pending.pop() {
        if let Some(text) = document.arena.kind_of::<TextNode>(node) {
            return document.text(text).to_owned();
        }
        pending.extend(document.children(node));
    }
    String::new()
}

fn tap(harness: &mut Harness, test_id: &str) {
    let at = harness.center(harness.find(test_id));
    harness.click(at);
    harness.settle();
}

#[test]
fn a_stacked_dock_bar_goes_back_switches_tabs_and_holds_the_tabs_actions() {
    let _root = crate::reactive::RootScope::new();
    let closed = create_signal(Vec::new());
    let open = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
        let layout = DockingLayout::new();
        view! {
            <dock::styled::Docking layout focus=FIRST>
                <DockPane id="tabs">
                    <DockTab id=FIRST title="Tab 1">
                        <Frame @test_id="content.1" />
                    </DockTab>
                    <ForEach keys={open}>|id: u64|
                        <DockTab
                            id
                            title={format!("Tab {id}")}
                            on_close={move || {
                                closed.update(|closed| closed.push(TabId::new(id)));
                                open.update(|open| open.retain(|other| *other != id));
                            }}
                            content={move || {
                                let act = Action::new(format!("action.{id}"), "Act", || {})
                                    .pin(Pin::Icon)
                                    .detached();
                                dock_menu(create_memo(move || vec![MenuNode::action(&act)]));
                                view! {
                                    <Frame @test_id={format!("content.{id}")} />
                                }
                            }}
                        />
                    </ForEach>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, VIEWPORT);
    harness.settle();
    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.back"),
        "the first tab shown has nowhere to go back to"
    );
    assert_eq!(
        count(&harness),
        "0",
        "the switcher shows even with nothing to close, and counts no tab that cannot be closed"
    );

    open.update(|open| open.push(2));
    harness.settle();
    open.update(|open| open.push(3));
    harness.settle();
    assert!(laid_out(&harness, "content.3"), "a tab opens on show");
    assert!(
        laid_out(&harness, "dock.back"),
        "a tab opened from another can go back to it"
    );
    assert_eq!(
        count(&harness),
        "2",
        "the count takes in every tab that can be closed, the one on show too"
    );
    let action = harness.rect(harness.find("dock.pin.action.3"));
    let content = harness.rect(harness.find("content.3"));
    assert!(
        action.bottom() <= content.top(),
        "the tab's own actions sit in the bar above it"
    );

    tap(&mut harness, "dock.back");
    assert_eq!(
        closed.get_untracked(),
        vec![TabId::new(3)],
        "back closes the tab it opened"
    );
    assert!(
        laid_out(&harness, "content.2"),
        "and shows the tab that opened it"
    );
    assert!(
        !laid_out(&harness, "dock.pin.action.3"),
        "a closed tab's actions leave the bar with it"
    );
    assert!(laid_out(&harness, "dock.pin.action.2"));

    tap(&mut harness, "dock.switch");
    assert!(
        laid_out(&harness, "dock.switcher.tab.1"),
        "the switcher lists every open tab"
    );
    tap(&mut harness, "dock.switcher.tab.1");
    assert!(laid_out(&harness, "content.1"), "choosing a tab shows it");
    assert!(
        !laid_out(&harness, "dock.switcher.tab.2"),
        "choosing a tab closes the switcher"
    );
    assert!(
        laid_out(&harness, "dock.back"),
        "a tab switched to can go back"
    );

    harness.key(Key::BrowserBack, Modifiers::NONE);
    harness.settle();
    assert!(
        laid_out(&harness, "content.2"),
        "the back gesture returns to the tab switched away from"
    );
    assert_eq!(
        closed.get_untracked(),
        vec![TabId::new(3)],
        "and keeps open the tab it leaves, which was only switched to"
    );

    tap(&mut harness, "dock.back");
    assert_eq!(closed.get_untracked(), vec![TabId::new(3), TabId::new(2)]);
    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.back"),
        "back has run out where it started"
    );
}
