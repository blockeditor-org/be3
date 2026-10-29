use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::{
    BeuiApp, ChildBlock, ChildBlockHandle, ChildContent, ChildMode, ChildState, ChildTarget,
    Editor, EditorHost, OwnedEditor,
};
use uuid::Uuid;

use crate::BeuiTest;

mod a_child_block_hears_what_its_childs_bar_asked_for;
mod a_child_block_reports_its_placement_and_follows_its_status;
mod a_floating_overlay_covers_only_the_children_painted_beneath_it;
mod a_joined_docks_layout_is_kept_on_this_device_and_every_instance_follows_it;
mod a_plugin_timer_runs_on_the_tests_clock;
mod a_shortcut_on_punctuation_reaches_the_editor;
mod ctrl_z_in_a_text_field_is_left_to_the_field;
mod ctrl_z_undoes_a_block_that_owns_its_frame_without_a_top_bar;
mod ctrl_z_undoes_the_block_through_the_frames_undo;
mod edits_from_elsewhere_land_in_order;
mod tapping_with_two_fingers_undoes_and_three_redoes;
mod the_frame_offers_one_menu_on_a_phone_and_a_desktop_and_runs_what_is_picked;
mod the_menu_offers_close_only_to_a_framed_child;
mod the_menu_opens_the_command_palette;
mod undo_in_the_menu_asks_the_host_for_a_block_it_cannot_open;

const SLIDE: Uuid = Uuid::from_u128(0x0001);
const SLIDE_TYPE: Uuid = Uuid::from_u128(0x0002);

struct ChildApp;

#[component]
impl BeuiApp for ChildApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Slide editor={editor} />
            </Root>
        }
    }
}

#[component]
fn Slide(editor: Editor) -> NodeId {
    let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
    view! {
        <Frame width=Size::Popover height={Size::Custom(180.0)}>
            <ChildBlock
                editor={editor}
                block={target}
                mode=ChildMode::Preview
            >|handle: ChildBlockHandle|
                <Status state={handle.state} />
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
        <Text @test_id={"child.status"}>{text}</Text>
    }
}

fn editor() -> BeuiTest<ChildApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4()))
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

fn undoable_editor() -> (BeuiTest<ChildApp>, Uuid) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<ChildApp>::new(OwnedEditor::new(host, block)).with_top_bar(false);
    test.set_histories([(
        block,
        block_editor_beui::BlockHistory {
            head: None,
            can_undo: true,
            can_redo: false,
        },
    )]);
    test.run();
    (test, block)
}
