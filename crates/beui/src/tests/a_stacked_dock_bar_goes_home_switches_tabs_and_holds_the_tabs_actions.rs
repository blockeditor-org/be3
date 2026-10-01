use super::*;
use crate::unstyled::{DockMode, DockState, TabId, dock_actions};

const HOME: TabId = TabId::new(1);

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
fn a_stacked_dock_bar_goes_home_switches_tabs_and_holds_the_tabs_actions() {
    let closed = Rc::new(RefCell::new(Vec::new()));
    let closing = closed.clone();
    let document = build(move || {
        let mut layout = DockState::new([HOME, TabId::new(2), TabId::new(3)]);
        layout.show(TabId::new(3));
        let (state, set_state) = create_signal(layout);
        view! {
            <styled::DockArea
                state={state}
                mode=DockMode::Stacked
                home={Some(HOME)}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                closable={Func::new(|tab: TabId| tab != HOME)}
                on_change={move |next: DockState| set_state.set(next)}
                on_close={move |tab: TabId| closing.borrow_mut().push(tab)}
            >
                {move |tab: TabId| {
                    let id = tab.value();
                    if tab != HOME {
                        dock_actions(view! {
                            <Frame @test_id={format!("action.{id}")} width=24.0 height=24.0 />
                        });
                    }
                    view! {
                        <Frame @test_id={format!("content.{id}")} />
                    }
                }}
            </styled::DockArea>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();

    assert!(laid_out(&harness, "content.3"));
    assert!(
        laid_out(&harness, "dock.back"),
        "a tab away from home can go back"
    );
    assert_eq!(count(&harness), "2", "the count leaves out the home tab");
    let action = harness.rect(harness.find("action.3"));
    let content = harness.rect(harness.find("content.3"));
    assert!(
        action.bottom() <= content.top(),
        "the tab's own actions sit in the bar above it"
    );

    tap(&mut harness, "dock.back");
    assert!(laid_out(&harness, "content.1"), "back shows the home tab");
    assert!(!laid_out(&harness, "content.3"));
    assert!(
        !laid_out(&harness, "dock.back"),
        "home has nowhere to go back to"
    );
    assert!(
        !laid_out(&harness, "action.3"),
        "a hidden tab's actions leave the bar with it"
    );

    tap(&mut harness, "dock.switch");
    assert!(
        !laid_out(&harness, "dock.switcher.tab.1"),
        "home is reached from its own row, not listed among the tabs"
    );
    tap(&mut harness, "dock.switcher.tab.2");
    assert!(laid_out(&harness, "content.2"), "choosing a tab shows it");
    assert!(laid_out(&harness, "action.2"));
    assert!(
        !laid_out(&harness, "dock.switcher.tab.3"),
        "choosing a tab closes the switcher"
    );

    tap(&mut harness, "dock.switch");
    tap(&mut harness, "dock.switcher.close.2");
    assert_eq!(*closed.borrow(), vec![TabId::new(2)]);
    assert_eq!(count(&harness), "1");
    assert!(
        laid_out(&harness, "content.1"),
        "closing the tab on show goes back to the one shown before it"
    );
    assert!(!laid_out(&harness, "dock.switcher.tab.2"));

    tap(&mut harness, "dock.switcher.tab.3");
    assert!(laid_out(&harness, "content.3"));
    harness.key(Key::BrowserBack, Modifiers::NONE);
    harness.settle();
    assert!(
        laid_out(&harness, "content.1"),
        "the back gesture goes home too"
    );
}
