use super::*;
use crate::reactive::*;
use crate::unstyled::{DockMode, DockPane, DockTab, DockingLayout, TabId, dock_actions};

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
    let document = build(move || {
        let layout = DockingLayout::new();
        view! {
            <styled::Docking layout mode=DockMode::Stacked focus=FIRST>
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
                                dock_actions(move || view! {
                                    <Frame
                                        @test_id={format!("action.{id}")}
                                        width=24.0
                                        height=24.0
                                    />
                                });
                                view! {
                                    <Frame @test_id={format!("content.{id}")} />
                                }
                            }}
                        />
                    </ForEach>
                </DockPane>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    assert!(laid_out(&harness, "content.1"));
    assert!(
        !laid_out(&harness, "dock.back"),
        "the first tab shown has nowhere to go back to"
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
        "1",
        "the count leaves out the tab on show and the tab that cannot be closed"
    );
    let action = harness.rect(harness.find("action.3"));
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
        !laid_out(&harness, "action.3"),
        "a closed tab's actions leave the bar with it"
    );
    assert!(laid_out(&harness, "action.2"));

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
