use be_sdk_beui::{
    BeuiRoot, ChildBlock, ChildBlockHandle, ChildContent, ChildMode, ChildState, ChildTarget,
    Editor, EditorHost, OwnedEditor,
};
use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use uuid::Uuid;

use crate::BeuiTest;
use be_sdk_beui::be_block::{DisplaySettings, Root as _};
use be_sdk_beui::be_block::{
    DisplaySettingsContent, Settings, SettingsContent, SettingsProfile, SettingsProfileContent,
};
use be_sdk_beui::{BlockInfo, BlockParent};

mod a_blocks_agent_is_made_rotated_and_removed_through_the_host;
mod a_child_block_hears_what_its_childs_bar_asked_for;
mod a_child_block_reports_its_placement_and_follows_its_status;
mod a_dock_bar_lifted_by_its_grip_draws_above_a_hosted_window;
mod a_fetched_body_reaches_the_editor_in_the_chunks_it_arrived_in;
mod a_floating_overlay_covers_only_the_children_painted_beneath_it;
mod a_hosted_window_keeps_its_grip_off_the_band_of_its_docks_bars;
mod a_hosted_window_keeps_its_place_when_a_dock_bar_moves_the_docks_area;
mod a_joined_dock_built_again_comes_back_in_the_layout_this_device_kept;
mod a_joined_docks_layout_is_kept_on_this_device_and_every_instance_follows_it;
mod a_layout_kept_while_joined_comes_back_in_a_standalone_dock;
mod a_pick_from_a_childs_menu_points_at_the_child_in_the_host;
mod a_placed_window_hears_its_status_and_is_asked_to_close;
mod a_popped_out_bar_stays_above_a_window_raised_over_it_until_covered;
mod a_shortcut_on_punctuation_reaches_the_editor;
mod a_standalone_docks_layout_is_kept_on_this_device_and_every_instance_follows_it;
mod a_watch_open_while_asked_starts_anew_each_time_it_opens;
mod an_actions_label_and_where_the_focus_is_reach_the_host;
mod an_app_timer_runs_on_the_tests_clock;
mod an_editor_describes_its_actions_to_the_host_and_runs_one_it_asks_for;
mod an_editor_opens_a_link_through_its_host;
mod an_editor_with_no_block_of_its_own_picks_into_unfiled;
mod an_editors_messages_to_the_shell_keep_the_order_it_made_them_in;
mod ctrl_z_in_a_text_field_is_left_to_the_field;
mod ctrl_z_undoes_a_block_that_owns_its_frame_without_a_top_bar;
mod ctrl_z_undoes_the_block_through_the_frames_undo;
mod dragging_a_popped_out_bars_grip_lifts_it_out_of_its_window_and_opens_no_menu;
mod edits_from_elsewhere_land_in_order;
mod narrowing_one_kind_of_block_watch_keeps_the_others;
mod tapping_with_two_fingers_undoes_and_three_redoes;
mod the_frame_offers_one_menu_on_a_phone_and_a_desktop_and_runs_what_is_picked;
mod the_menu_offers_close_only_to_a_framed_child;
mod the_menu_opens_the_command_palette;
mod undo_at_a_session_restart_says_so_and_offers_version_history;
mod undo_in_the_menu_asks_the_host_for_a_block_it_cannot_open;
mod with_a_host_palette_the_menu_asks_the_host_for_it;

const SLIDE: Uuid = Uuid::from_u128(0x0001);
const SLIDE_TYPE: Uuid = Uuid::from_u128(0x0002);

struct ChildRoot;

#[component]
impl BeuiRoot for ChildRoot {
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

fn editor() -> BeuiTest<ChildRoot> {
    BeuiTest::editing(Uuid::new_v4())
}

fn status(test: &BeuiTest<ChildRoot>) -> String {
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

fn undoable_editor() -> (BeuiTest<ChildRoot>, Uuid) {
    let block = Uuid::new_v4();
    let mut test = BeuiTest::<ChildRoot>::editing(block).with_top_bar(false);
    test.set_histories([(
        block,
        be_sdk_beui::BlockHistory {
            head: None,
            can_undo: true,
            can_redo: false,
            ..Default::default()
        },
    )]);
    test.run();
    (test, block)
}

fn settings_here<A: BeuiRoot>(test: &mut BeuiTest<A>, display: DisplaySettingsContent) -> Uuid {
    let client = test.host().client_id();
    let (settings, profile, held) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let store = test.store();
    store.add_block(BlockInfo::new(
        settings,
        Settings::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.add_block(BlockInfo::new(
        profile,
        SettingsProfile::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    let mut own = SettingsContent::default();
    own.apply(&Settings::add_profile(client, profile));
    store.hold(Some(settings), own);
    let mut kinds = SettingsProfileContent::default();
    kinds.apply(&SettingsProfile::set_kind(
        DisplaySettings::CONTENT_TYPE,
        held,
    ));
    store.hold(Some(profile), kinds);
    store.hold(Some(held), display);
    test.run();
    held
}

const STANDALONE_TYPE: Uuid = Uuid::from_u128(0x0b43);

thread_local! {
    static STANDALONE_LAYOUTS: std::cell::RefCell<Vec<beui::dock::DockingLayout<u64>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

struct StandaloneRoot;

#[component]
impl BeuiRoot for StandaloneRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        use beui::dock::{DockBar, DockBarPlacement, DockEdge};
        use beui::dock::{DockPane, DockSplit, DockTab, Docking, DockingLayout};
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        STANDALONE_LAYOUTS.with(|layouts| layouts.borrow_mut().push(layout));
        view! {
            <Root>
                <Docking layout>
                    <DockSplit id="main">
                        <DockPane id="stage" bare=true>
                            <DockTab id=1u64 title="Stage">
                                <Frame />
                            </DockTab>
                        </DockPane>
                        <DockPane id="side">
                            <DockTab id=2u64 title="Notes">
                                <Frame />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                    <DockBar
                        id=3u64
                        parent=1u64
                        placement=DockBarPlacement::Over(DockEdge::Top, beui::dock::DockAlign::Center)
                    >
                        <Frame width={Size::Custom(120.0)} height={Size::Custom(24.0)} />
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

fn standalone(
    display: DisplaySettingsContent,
    docked: bool,
) -> (
    BeuiTest<StandaloneRoot>,
    Uuid,
    beui::dock::DockingLayout<u64>,
) {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(STANDALONE_TYPE);
    let mut test = BeuiTest::<StandaloneRoot>::new(OwnedEditor::new(host, Uuid::new_v4()));
    if docked {
        test = test.docked();
    }
    let held = settings_here(&mut test, display);
    let layout = STANDALONE_LAYOUTS
        .with(|layouts| layouts.borrow().last().copied())
        .expect("the editor made its dock");
    (test, held, layout)
}

fn kept_dock<A: BeuiRoot>(test: &BeuiTest<A>, display: Uuid) -> (String, String) {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .iter()
        .map(|(key, layout)| (key.clone(), layout.clone()))
        .next()
        .expect("the editor keeps its dock's layout on this device")
}
