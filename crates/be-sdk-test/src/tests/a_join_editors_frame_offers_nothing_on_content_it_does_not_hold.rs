use be_sdk_beui::beui::styled::Root;
use beui::Vec2;

use super::*;

struct WorldRoot;

#[component]
impl BeuiRoot for WorldRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        view! {
            <Root>
                <Text>"A world"</Text>
            </Root>
        }
    }

    fn exporters() -> Vec<be_sdk_beui::Exporter> {
        vec![
            be_sdk_beui::ExporterOf::<be_type_text::TextContent, ()>::new("copy", |_| {
                |_: &()| Err(be_sdk_beui::beui::reactive::Pending::Loading)
            })
            .into(),
        ]
    }
}

/// An editor opened as a Join member holds none of its block's content
/// (guides/blocks/hosted.md, "Access"), so its frame offers no undo, redo, rename or share,
/// or export, in the menu or the palette, where an editor of the block offers each.
#[test]
fn a_join_editors_frame_offers_nothing_on_content_it_does_not_hold() {
    let content = [
        "editor.undo",
        "editor.redo",
        "editor.rename",
        "editor.share",
        "editor.export.copy",
    ];
    let offered = |test: &BeuiTest<WorldRoot>| -> Vec<String> {
        let mut ids = test.menu_ids();
        ids.extend(test.described_actions().iter().map(|action| action.id.clone()));
        content
            .iter()
            .filter(|id| ids.iter().any(|offered| offered == *id))
            .map(|id| (*id).to_owned())
            .collect()
    };

    let editing = BeuiTest::<WorldRoot>::editing(Uuid::new_v4())
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert_eq!(offered(&editing), content, "an editor of the block offers each");
    drop(editing);

    let joined = BeuiTest::<WorldRoot>::joined(Uuid::new_v4())
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    assert!(
        !joined.menu_ids().is_empty(),
        "a Join editor's frame still has a menu"
    );
    assert_eq!(offered(&joined), Vec::<String>::new(), "a Join editor offers none");
}

test_editor!(WorldRoot);
