use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;
use block_editor_beui::{
    BeuiApp, ChildBlock, ChildBlockHandle, ChildContent, ChildMode, ChildState, ChildTarget,
    Editor, EditorHost, OwnedEditor,
};
use uuid::Uuid;

use crate::BeuiTest;
use block_editor_beui::be_block::{DisplaySettings, Root as _};
use block_editor_beui::be_block::{
    DisplaySettingsContent, Settings, SettingsContent, SettingsProfile, SettingsProfileContent,
};
use block_editor_beui::{BlockInfo, BlockParent};

mod a_child_block_hears_what_its_childs_bar_asked_for;
mod a_child_block_reports_its_placement_and_follows_its_status;
mod a_dock_bar_lifted_by_its_grip_draws_above_a_hosted_window;
mod a_floating_overlay_covers_only_the_children_painted_beneath_it;
mod a_hosted_window_is_kept_off_the_band_of_its_docks_bars;
mod a_hosted_window_keeps_its_place_when_a_dock_bar_moves_the_docks_area;
mod a_joined_dock_built_again_comes_back_in_the_layout_this_device_kept;
mod a_joined_docks_layout_is_kept_on_this_device_and_every_instance_follows_it;
mod a_layout_kept_while_joined_comes_back_in_a_standalone_dock;
mod a_plugin_timer_runs_on_the_tests_clock;
mod a_shortcut_on_punctuation_reaches_the_editor;
mod a_standalone_docks_layout_is_kept_on_this_device_and_every_instance_follows_it;
mod ctrl_z_in_a_text_field_is_left_to_the_field;
mod ctrl_z_undoes_a_block_that_owns_its_frame_without_a_top_bar;
mod ctrl_z_undoes_the_block_through_the_frames_undo;
mod dragging_a_popped_out_bars_grip_lifts_it_out_of_its_window_and_opens_no_menu;
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
    BeuiTest::editing(Uuid::new_v4())
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
    let mut test = BeuiTest::<ChildApp>::editing(block).with_top_bar(false);
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

fn settings_here<A: BeuiApp>(test: &mut BeuiTest<A>, display: DisplaySettingsContent) -> Uuid {
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
    static STANDALONE_LAYOUTS: std::cell::RefCell<Vec<beui::dock::styled::DockingLayout<u64>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

struct StandaloneApp;

#[component]
impl BeuiApp for StandaloneApp {
    #[component]
    fn View(editor: Editor) -> Root {
        use beui::dock::styled::{DockBar, DockBarPlacement, DockEdge};
        use beui::dock::styled::{DockPane, DockSplit, DockTab, Docking, DockingLayout};
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
                        placement=DockBarPlacement::Over(DockEdge::Top, beui::dock::unstyled::DockAlign::Center)
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
    BeuiTest<StandaloneApp>,
    Uuid,
    beui::dock::styled::DockingLayout<u64>,
) {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(STANDALONE_TYPE);
    let mut test = BeuiTest::<StandaloneApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    if docked {
        test = test.docked();
    }
    let held = settings_here(&mut test, display);
    let layout = STANDALONE_LAYOUTS
        .with(|layouts| layouts.borrow().last().copied())
        .expect("the editor made its dock");
    (test, held, layout)
}

fn kept_dock<A: BeuiApp>(test: &BeuiTest<A>, display: Uuid) -> (String, String) {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .iter()
        .map(|(key, layout)| (key.clone(), layout.clone()))
        .next()
        .expect("the editor keeps its dock's layout on this device")
}
