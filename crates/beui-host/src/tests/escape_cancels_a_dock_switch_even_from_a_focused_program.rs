use super::*;
use crate::ForwardedInput;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn escape_cancels_a_dock_switch_even_from_a_focused_program() {
    let _root = crate::reactive::RootScope::new();
    let layout = DockingLayout::<u64>::new();
    let program = NodeRef::new();
    let heard = create_signal(Vec::<ForwardedInput>::new());
    let document = build_node(move || {
        view! {
            <dock::styled::Docking layout={layout}>
                <DockPane id="tabs">
                    <DockTab id=1u64 title="One">
                        <Interactive
                            @node_ref=&program
                            focusable=true
                            on_forward={move |input| heard.update(|heard| heard.push(input))}
                        >
                            <Frame />
                        </Interactive>
                    </DockTab>
                    <DockTab id=2u64 title="Two">
                        <Frame />
                    </DockTab>
                </DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    layout.show(&2);
    layout.show(&1);
    harness.frame(Vec::new());
    harness.document_mut().focus_focusable(program.get());
    layout.begin_switch(false);
    harness.frame(Vec::new());
    assert!(layout.switching());
    heard.set(Vec::new());

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        !layout.switching(),
        "Escape cancels the switch though the switcher holds no focus"
    );
    let escapes = heard.with_untracked(|heard| {
        heard
            .iter()
            .flat_map(|input| flattened(input.clone()))
            .filter(|event| matches!(event, Event::Key { press, .. } if press.key == Key::Escape))
            .count()
    });
    assert_eq!(escapes, 0, "and the focused program never hears it");
}
