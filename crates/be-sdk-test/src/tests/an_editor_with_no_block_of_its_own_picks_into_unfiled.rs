use beui::styled::Root;

use super::*;

struct Picking;

#[component]
impl BeuiRoot for Picking {
    #[component]
    fn View(editor: Editor) -> Root {
        editor.pick_block(
            be_sdk_beui::BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: Vec::new(),
                templates: false,
                place: None,
            },
            None::<beui::Rect>,
            |_| {},
        );
        view! {
            <Root>
                <Text>"Picking"</Text>
            </Root>
        }
    }
}

#[test]
fn an_editor_with_no_block_of_its_own_picks_into_unfiled() {
    let mut test: BeuiTest<Picking> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::nil()));
    test.settle();

    let picker = test
        .placed_picker()
        .expect("asking for a block places a picker");
    let be_sdk_api::ChildContent::Picker { parent, .. } = picker.content else {
        unreachable!("placed_picker finds only a picker");
    };
    assert_eq!(
        parent,
        be_sdk_api::BlockLocation::Unfiled,
        "with no block of its own to put a new one in, the editor asks for Unfiled"
    );
}
