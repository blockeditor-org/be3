use super::*;
use crate::KeyPress;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::CommandPaletteApi;
use crate::primitives::List;

#[test]
fn the_command_palette_runs_the_action_it_is_filtered_to_and_greys_disabled_ones() {
    let _root = crate::reactive::RootScope::new();
    let ran = create_signal(Vec::new());
    let palette = NodeRef::new();
    let document = build_node(move || {
        Action::new("test.lock", "Lock selection", move || {
            ran.update(|ran| ran.push("lock"))
        })
        .enabled(false)
        .register();
        Action::new("test.zoom-in", "Zoom in", move || {
            ran.update(|ran| ran.push("zoom-in"))
        })
        .register();
        Action::new("test.reset", "Reset zoom", move || {
            ran.update(|ran| ran.push("reset"))
        })
        .register();
        let open = create_signal(false);
        on_shortcut(move |press: KeyPress| {
            let wanted = press.pressed
                && press.key == Key::P
                && press.modifiers.ctrl
                && press.modifiers.shift;
            if wanted {
                open.set(true);
            }
            wanted
        });
        view! {
            <List spacing=0.0>
                <unstyled::Button @test_id={"button"}>
                    <ButtonFace label="Canvas" />
                </unstyled::Button>
                <CommandPalette @node_ref=&palette open />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let button = harness.find("button");
    harness.click(harness.center(button));

    harness.key(
        Key::P,
        Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    );
    let palette = palette.get();
    assert_eq!(
        harness.document().api::<CommandPaletteApi>(palette).shown(),
        vec!["test.zoom-in", "test.reset", "test.lock"],
        "disabled actions are listed last"
    );
    assert_eq!(
        harness
            .document()
            .api::<CommandPaletteApi>(palette)
            .highlighted()
            .as_deref(),
        Some("test.zoom-in")
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<CommandPaletteApi>(palette)
            .highlighted()
            .as_deref(),
        Some("test.reset"),
        "the highlight skips a disabled action"
    );
    let lock = harness
        .document()
        .api::<CommandPaletteApi>(palette)
        .row("test.lock")
        .expect("lock is shown");
    harness.click(harness.center(lock));
    assert!(
        ran.with_untracked(|ran| ran.is_empty()),
        "a disabled action does not run when clicked"
    );

    harness.type_text("zoom");
    assert_eq!(
        harness.document().api::<CommandPaletteApi>(palette).shown(),
        vec!["test.zoom-in", "test.reset"]
    );
    harness.type_text(" in");
    assert_eq!(
        harness.document().api::<CommandPaletteApi>(palette).shown(),
        vec!["test.zoom-in"]
    );
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(ran.get_untracked(), vec!["zoom-in"]);
    assert_eq!(
        with_installed(harness.document_mut(), |document| document.focused_node()),
        Some(button),
        "closing the palette returns the focus to where it was"
    );
}
