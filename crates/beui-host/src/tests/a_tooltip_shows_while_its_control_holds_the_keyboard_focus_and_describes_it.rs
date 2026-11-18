use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{Tooltip, TooltipHandle};

#[test]
fn a_tooltip_shows_while_its_control_holds_the_keyboard_focus_and_describes_it() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Tooltip
                    label="Adds a root block"
                    content={move |handle: TooltipHandle| {
                        view! {
                            <Text @test_id="tip">{handle.label}</Text>
                        }
                    }}
                >
                    <Button>
                        <Text>"Add"</Text>
                    </Button>
                </Tooltip>
                <Button>
                    <Text>"Other"</Text>
                </Button>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.document().find_test_id("tip"), None);
    let described = harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == accesskit::Role::Button && node.label().is_none())
        .and_then(|node| node.description().map(str::to_owned));
    assert_eq!(
        described.as_deref(),
        Some("Adds a root block"),
        "the tooltip's label describes the button to assistive technology while it is hidden"
    );

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("tip").is_some(),
        "the keyboard's focus shows the tooltip at once, with no dwell"
    );

    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().find_test_id("tip"),
        None,
        "moving the focus on hides it"
    );
}
