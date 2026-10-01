use super::*;
use crate::KeyPress;
use crate::reactive::{Action, NodeRef, build, create_signal, on_shortcut, view};
use crate::styled::CommandPalette;
use crate::unstyled::{command_palette_highlighted, command_palette_row, command_palette_shown};

#[test]
fn the_command_palette_runs_the_action_it_is_filtered_to_and_greys_disabled_ones() {
    let ran = Rc::new(RefCell::new(Vec::new()));
    let (zoom, reset, locked) = (ran.clone(), ran.clone(), ran.clone());
    let palette = NodeRef::new();
    let held = palette.clone();
    let document = build(move || {
        Action::new("test.lock", "Lock selection", move || {
            locked.borrow_mut().push("lock")
        })
        .enabled(false)
        .register();
        Action::new("test.zoom-in", "Zoom in", move || {
            zoom.borrow_mut().push("zoom-in")
        })
        .register();
        Action::new("test.reset", "Reset zoom", move || {
            reset.borrow_mut().push("reset")
        })
        .register();
        let (open, set_open) = create_signal(false);
        let opening = set_open.clone();
        on_shortcut(move |press: KeyPress| {
            let wanted = press.pressed
                && press.key == Key::P
                && press.modifiers.ctrl
                && press.modifiers.shift;
            if wanted {
                opening.set(true);
            }
            wanted
        });
        view! {
            <List spacing=0.0>
                <unstyled::Button @test_id={"button"}>
                    <ButtonFace label="Canvas" />
                </unstyled::Button>
                <CommandPalette @node_ref=&held open on_close={move || set_open.set(false)} />
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
        command_palette_shown(harness.document(), palette),
        vec!["test.zoom-in", "test.reset", "test.lock"],
        "disabled actions are listed last"
    );
    assert_eq!(
        command_palette_highlighted(harness.document(), palette).as_deref(),
        Some("test.zoom-in")
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        command_palette_highlighted(harness.document(), palette).as_deref(),
        Some("test.reset"),
        "the highlight skips a disabled action"
    );
    let lock =
        command_palette_row(harness.document(), palette, "test.lock").expect("lock is shown");
    harness.click(harness.center(lock));
    assert!(
        ran.borrow().is_empty(),
        "a disabled action does not run when clicked"
    );

    harness.type_text("zoom");
    assert_eq!(
        command_palette_shown(harness.document(), palette),
        vec!["test.zoom-in", "test.reset"]
    );
    harness.type_text(" in");
    assert_eq!(
        command_palette_shown(harness.document(), palette),
        vec!["test.zoom-in"]
    );
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*ran.borrow(), vec!["zoom-in"]);
    assert_eq!(
        with_installed(harness.document_mut(), |document| document.focused_node()),
        Some(button),
        "closing the palette returns the focus to where it was"
    );
}
