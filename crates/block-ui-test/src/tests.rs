use beui::NodeId;
use beui::reactive::{Frame, ReadSignal, Text, component, create_memo, view};
use block_editor_beui::be_block::{BlockContent, FileTreeContent};
use block_editor_beui::{
    BeuiApp, ChildBlock, ChildBlockHandle, ChildMode, ChildState, ChildTarget, Editor, EditorHost,
};
use uuid::Uuid;

use crate::{BeuiTest, ContentStore};

mod a_child_block_reports_its_placement_and_follows_its_status;
mod a_shortcut_on_punctuation_reaches_the_editor;
mod clearing_the_name_gives_the_block_back_its_derived_name;
mod ctrl_z_in_a_text_field_is_left_to_the_field;
mod ctrl_z_undoes_the_block_through_the_top_bar;
mod the_top_bar_offers_close_only_to_a_framed_child;
mod the_top_bar_renames_its_block;
mod undo_in_the_top_bar_asks_the_host_for_a_block_it_cannot_open;

const SLIDE: Uuid = Uuid::from_u128(0x0001);
const FILE_TREE: Uuid = FileTreeContent::CONTENT_TYPE;
const SLIDE_TYPE: Uuid = Uuid::from_u128(0x0002);

struct ChildApp;

impl BeuiApp for ChildApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Slide editor={editor} />
        }
    }
}

#[component]
fn Slide(editor: Editor) -> NodeId {
    let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
    view! {
        <Frame width=320.0 height=180.0>
            <ChildBlock editor={editor} block={target} mode=ChildMode::Preview>
                {move |handle: ChildBlockHandle| view! {
                    <Status state={handle.state} />
                }}
            </ChildBlock>
        </Frame>
    }
}

#[component]
fn Status(state: ReadSignal<ChildState>) -> NodeId {
    let text = create_memo(move || {
        state.with(|state| match (state.placed, state.available) {
            (false, _) => "unplaced".to_owned(),
            (true, false) => "waiting".to_owned(),
            (true, true) => "available".to_owned(),
        })
    });
    view! {
        <Text string={text} @test_id={"child.status"} />
    }
}

fn editor() -> BeuiTest<ChildApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    BeuiTest::new(Editor::new(host, Uuid::new_v4()))
}

fn status(test: &BeuiTest<ChildApp>) -> String {
    let node = test
        .document()
        .find_test_id("child.status")
        .expect("the status text is in the tree");
    test.document()
        .node_detail(node)
        .expect("the status text has content")
        .trim_matches('"')
        .to_owned()
}

fn named_editor() -> (BeuiTest<ChildApp>, ContentStore, Uuid) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(FILE_TREE);
    let mut test = BeuiTest::new(Editor::new(host, block)).with_top_bar(false);
    test.run();
    let store = test.store();
    (test, store, block)
}

fn name(store: &ContentStore, block: Uuid) -> Option<String> {
    store
        .block(block)
        .filter(|info| info.named_by_hand)
        .and_then(|info| info.name)
}

fn shown_name(test: &BeuiTest<ChildApp>) -> String {
    let input = test
        .document()
        .find_test_id("editor.name")
        .expect("the top bar has a name field");
    beui::styled::text_input_value(test.document(), input)
}

fn undoable_editor() -> (BeuiTest<ChildApp>, Uuid) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ChildApp>::new(Editor::new(host, block)).with_top_bar(false);
    test.set_histories([(
        block,
        block_editor_beui::BlockHistory {
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();
    (test, block)
}
