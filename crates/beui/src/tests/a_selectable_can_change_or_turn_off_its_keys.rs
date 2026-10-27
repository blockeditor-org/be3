use super::*;
use crate::native::with_reactive_scope;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_selectable_can_change_or_turn_off_its_keys() {
    let _root = crate::reactive::RootScope::new();
    let (region, text) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText
                @node_ref=&region
                copy_key={Some(Chord::ctrl(Key::C).shift())}
                select_all_key=None
            >
                <Text @node_ref=&text>"echo hello"</Text>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(text.get()));
    harness.frame(Vec::new());
    let copy = |harness: &mut Harness, modifiers: Modifiers| {
        harness.frame(pressed_with(Key::C, modifiers)).copied_text
    };
    let ctrl_shift = Modifiers {
        shift: true,
        ..Modifiers::CTRL
    };

    harness.frame(pressed_with(Key::A, Modifiers::CTRL));
    assert_eq!(
        copy(&mut harness, ctrl_shift),
        None,
        "Ctrl+A selects nothing once select all has no key"
    );

    let state = harness
        .document()
        .api::<crate::styled::SelectableState>(region.get());
    with_reactive_scope(harness.document_mut(), || {
        crate::selectable::select_all(state)
    });
    assert_eq!(
        copy(&mut harness, Modifiers::CTRL),
        None,
        "Ctrl+C is left to what is around it"
    );
    assert_eq!(
        copy(&mut harness, ctrl_shift).as_deref(),
        Some("echo hello"),
        "the key it was given copies"
    );
}
