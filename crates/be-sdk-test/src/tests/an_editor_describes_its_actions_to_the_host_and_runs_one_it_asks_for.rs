use beui::icons::ICON_SPARKLES;
use beui::styled::Root;
use beui::{Key, KeyChord, Modifiers};

use super::*;

struct ActionsRoot;

#[component]
impl BeuiRoot for ActionsRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Tidy editor={editor} />
            </Root>
        }
    }
}

#[component]
fn Tidy(editor: Editor) -> NodeId {
    let _ = editor;
    let tidied = create_signal(String::from("messy"));
    let allowed = create_signal(true);
    Action::new("tidy.up", "Tidy up", move || tidied.set("tidy".to_owned()))
        .glyph(ICON_SPARKLES)
        .shortcut(Chord::ctrl(Key::T))
        .enabled(allowed)
        .register();
    view! {
        <List spacing=Space::Md>
            <Text @test_id={"tidy.state"}>{tidied}</Text>
            <Button @test_id={"tidy.forbid"} label="Forbid" on_click={move || allowed.set(false)} />
        </List>
    }
}

#[test]
fn an_editor_describes_its_actions_to_the_host_and_runs_one_it_asks_for() {
    let mut test = BeuiTest::<ActionsRoot>::editing(Uuid::new_v4());
    let tidy = test.described_action("tidy.up");
    assert_eq!(tidy.label, "Tidy up");
    assert_eq!(tidy.glyph.as_deref(), ICON_SPARKLES.name());
    assert_eq!(
        tidy.shortcut,
        Some(KeyChord::new(
            Key::T,
            Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            }
        )),
        "the shortcut travels as a chord"
    );
    assert!(tidy.enabled && tidy.live);
    assert!(
        test.described_actions()
            .iter()
            .any(|action| action.id == "editor.palette"),
        "the frame's own actions are described too, though nothing asked the app to describe"
    );

    test.run_action("tidy.up");
    assert_eq!(test.label("tidy.state"), "tidy", "the host runs it by id");

    test.click("tidy.forbid");
    test.settle();
    let tidy = test.described_action("tidy.up");
    assert!(
        !tidy.enabled,
        "the description follows the action's enabled state: {tidy:?}"
    );
}
