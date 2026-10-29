use block_editor_beui::be_block::{EditorView, EditorViewContent, ViewState};
use workspace_ui_types::{FolderContent, WorkspaceContent};

use block_editor_beui::beui::{NodeId, Rect, ScrollDelta, Vec2};

use block_editor_beui::beui::native::Document;
use block_editor_beui::{
    BlockParent, ChildContent, EditorHost, OwnedEditor, WindowAction, WindowsWatch,
};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::WorkspaceUiApp;
use crate::picker::PickerApp;

mod a_block_in_a_tab_points_its_dialogs_at_the_tab;
mod a_block_opened_while_another_is_shown_gets_its_own_tab;
mod a_block_opens_in_the_editor_it_keeps_for_itself;
mod a_block_tab_asks_its_editor_for_the_top_bar;
mod a_block_tab_dragged_onto_the_sidebar_stays_with_the_blocks;
mod a_block_tab_unlocked_from_its_grip_moves_into_the_sidebar;
mod a_block_tabs_menu_previews_its_access_and_comes_back;
mod a_closed_tab_gives_up_its_view_block;
mod a_deleted_block_says_so_above_it_and_can_be_restored;
mod a_failed_creation_is_shown_and_dismissing_it_cancels_the_pick;
mod a_file_with_no_editor_of_its_own_opens_in_the_program_for_its_extension;
mod a_folder_chosen_on_the_location_page_holds_the_block_made_there;
mod a_hidden_window_and_its_dialog_have_no_tab_until_shown;
mod a_host_panel_the_host_asks_for_is_placed_in_its_own_window;
mod a_modal_window_sends_presses_on_its_parent_to_itself;
mod a_phone_shows_one_file_at_a_time_and_the_dock_bar_goes_back_or_switches;
mod a_presentation_offers_its_slide_templates_first;
mod a_profile_keeps_its_tabs_as_view_blocks_and_reopens_them;
mod a_reopened_phone_goes_back_through_the_files_in_the_order_they_were_shown;
mod a_reopened_tab_shows_its_saved_title_before_its_block_is_known;
mod a_shown_block_is_remembered_in_the_recents;
mod a_shown_block_is_reported_as_focused;
mod a_template_of_the_workspace_is_copied_into_place;
mod a_template_without_a_dialog_is_created_and_answers_the_pick;
mod a_window_tab_is_locked_to_the_blocks_area_like_a_block_tab;
mod a_window_tab_shows_the_icon_its_program_set;
mod a_window_that_stops_responding_offers_to_wait_or_force_close;
mod a_window_the_host_makes_fullscreen_covers_the_workspace_until_it_leaves;
mod a_window_the_host_raises_brings_its_hidden_tab_forward;
mod a_window_the_host_runs_gets_a_tab_until_its_program_closes_it;
mod a_window_with_a_parent_hangs_over_it_and_hides_with_its_tab;
mod a_workspace_opens_on_its_landing_page;
mod an_import_into_a_folder_is_made_inside_it;
mod an_import_keeps_the_location_it_started_with;
mod an_open_menu_is_withheld_from_the_block_under_it;
mod an_unlocked_tab_moved_into_the_sidebar_is_still_there_when_the_workspace_reopens;
mod back_asks_a_creation_dialog_to_dismiss_itself_first;
mod block_type_colors_are_shown_in_the_add_menu_and_full_list;
mod block_type_colors_are_shown_on_a_phone;
mod closing_a_window_tab_asks_its_program_to_close;
mod closing_the_only_tab_leaves_the_blank_workspace;
mod creating_from_a_dialog_commits_the_creation_at_once;
mod crossing_the_phone_width_keeps_the_block_on_show;
mod escape_closes_the_add_menu_and_only_its_cards_ask_to_be_framed;
mod importing_a_block_file_in_the_picker_answers_the_pick;
mod linking_a_block_in_the_picker_answers_the_pick;
mod on_a_desktop_a_touch_opens_the_add_menu_as_a_sheet_and_a_mouse_as_a_dropdown;
mod on_a_phone_more_blocks_takes_over_the_menu_sheet_in_place;
mod on_a_phone_tapping_a_files_name_opens_its_rename_sheet;
mod on_a_phone_the_menu_and_its_location_page_rise_as_a_sheet;
mod on_a_phone_the_more_sheet_keeps_its_cancel_below_the_list;
mod on_a_wide_phone_the_more_sheet_fills_the_screen_width;
mod renaming_a_block_the_host_asks_about_saves_its_name;
mod revision_history_follows_the_active_file_and_previews_it;
mod revision_history_has_no_file_for_a_native_window;
mod revision_history_keeps_its_list_in_place_while_a_save_reloads_it;
mod revision_history_on_a_phone_and_late_replies;
mod revision_history_previews_a_file_on_a_phone;
mod searching_the_link_tree_keeps_matches_and_the_folders_above_them;
mod sharing_a_block_grants_the_people_added_and_lists_them_again;
mod showing_a_block_with_another_editor_switches_its_tab;
mod switching_from_a_block_to_a_host_panel_keeps_the_panel_on_show;
mod the_back_gesture_on_a_phone_leaves_a_file_for_the_files;
mod the_blocks_area_splits_into_two_with_block_tabs_in_each;
mod the_link_card_shows_where_the_chosen_block_lives_and_links_it;
mod the_location_page_shows_the_folders_as_a_tree_with_the_chosen_one_checked;
mod the_picker_never_offers_a_deleted_block_to_link;
mod the_rename_card_sets_a_blocks_name_icon_and_color;
mod the_share_dialog_lists_again_when_a_contact_is_made;
mod the_share_dialog_says_where_each_persons_access_comes_from;
mod widening_the_phone_with_its_switcher_open_keeps_the_workspace;

const MAX_TAB: u64 = 64;
const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);
const TEMPLATE_EDITOR: Uuid = Uuid::from_u128(0x7465_6d70_6c61_7465_2d65_6469_746f_7231);
const TEMPLATE_TYPE: Uuid = Uuid::from_u128(0x7465_6d70_6c61_7465_2d74_7970_6531_0000);

struct Fixture<A: block_editor_beui::BeuiApp = WorkspaceUiApp> {
    test: BeuiTest<A>,
    host: EditorHost,
}

impl<A: block_editor_beui::BeuiApp> Fixture<A> {
    fn settle(&mut self) {
        self.test.run();
    }

    fn shown(&self) -> Vec<Uuid> {
        self.test
            .children()
            .iter()
            .filter_map(|placement| placement.content.block_id())
            .map(Uuid::from_bytes)
            .collect()
    }

    fn focused(&self) -> Option<Uuid> {
        self.host.focused_block().block_id
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| text_within(&document, root, words).is_some())
    }

    fn click_text(&mut self, words: &str) {
        let center = self.text_rect(words).center();
        self.test.click(center);
        self.settle();
    }

    fn text_rect(&self, words: &str) -> Rect {
        let document = self.test.document();
        let node = document
            .root()
            .and_then(|root| text_within(&document, root, words))
            .unwrap_or_else(|| panic!("{words:?} is on screen"));
        document.node_rect(node).expect("the text is laid out")
    }

    fn drag_text(&mut self, words: &str, to: block_editor_beui::beui::Pos2) {
        let from = self.text_rect(words).center();
        self.test.drag(from, to);
        self.settle();
    }

    fn show_history(&mut self) {
        let document = self.test.document();
        let settings = document
            .root()
            .and_then(|root| text_within(&document, root, "Settings"))
            .expect("the sidebar has a Settings tab");
        let at = document
            .node_rect(settings)
            .expect("the tab is laid out")
            .center();
        drop(document);
        self.test.step(vec![
            block_editor_beui::beui::Input::PointerMoved(at),
            block_editor_beui::beui::Input::Scroll(ScrollDelta::Pixels(Vec2::new(-200.0, 0.0))),
        ]);
        self.test
            .step(vec![block_editor_beui::beui::Input::ScrollEnded]);
        self.settle();
        self.click_text("History");
    }

    fn open_tabs(&self) -> usize {
        self.tab_closes().len()
    }

    fn close_active_tab(&mut self) {
        let cross = self
            .tab_closes()
            .first()
            .copied()
            .expect("an open tab can be closed");
        self.test.click(cross.center());
        self.settle();
    }

    fn tab_closes(&self) -> Vec<Rect> {
        let document = self.test.document();
        (0..MAX_TAB)
            .filter_map(|tab| document.find_test_id(&format!("dock.tab.{tab}.close")))
            .filter_map(|close| document.node_rect(close))
            .collect()
    }
}

fn text_within(document: &Document, id: NodeId, words: &str) -> Option<NodeId> {
    if document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
    {
        return Some(id);
    }
    document
        .children(id)
        .into_iter()
        .find_map(|child| text_within(&document, child, words))
}

fn window(id: u64, title: &str, parent: Option<u64>) -> block_editor_beui::HostWindow {
    block_editor_beui::HostWindow {
        id: block_editor_beui::HostWindowId(id),
        kind: block_editor_beui::WindowKind::Window,
        shown: true,
        title: title.to_owned(),
        app_id: "test".to_owned(),
        parent: parent.map(block_editor_beui::HostWindowId),
        modal: false,
        size: block_editor_beui::beui::Vec2::new(320.0, 200.0),
        fullscreen: None,
        responding: true,
        focused: false,
        icon: None,
        raised: 0,
        origin: Default::default(),
        note: None,
    }
}

fn placed_windows(fixture: &Fixture) -> Vec<(u64, Rect)> {
    fixture
        .test
        .children()
        .iter()
        .filter_map(|placement| match placement.content {
            ChildContent::Window(window) => Some((window.0, placement.rect)),
            _ => None,
        })
        .collect()
}

fn editor() -> (Fixture, Uuid) {
    editor_sized(None)
}

fn editor_sized(size: Option<Vec2>) -> (Fixture, Uuid) {
    open_editor(size, None)
}

fn profiled(layout: Option<ViewState>) -> (Fixture, Uuid) {
    profiled_sized(None, layout)
}

fn profiled_sized(size: Option<Vec2>, layout: Option<ViewState>) -> (Fixture, Uuid) {
    let mut profile = EditorView::document(None);
    if let Some(layout) = layout {
        let edit = profile
            .root()
            .set_state("layout", Some(&layout), 1, Uuid::nil());
        profile.apply(&edit);
    }
    open_editor(size, Some(profile))
}

fn open_editor(size: Option<Vec2>, profile: Option<EditorViewContent>) -> (Fixture, Uuid) {
    let workspace = Uuid::from_u128(0x3a_0001);
    let opened = Uuid::from_u128(0x3a_0002);
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0x3a_0003));
    if profile.is_some() {
        host.set_view_block(Some(workspace));
    }
    let editor = OwnedEditor::new(host.clone(), workspace);
    let mut fixture = Fixture {
        test: match size {
            Some(size) => BeuiTest::new(editor).with_size(size),
            None => BeuiTest::new(editor),
        },
        host,
    };
    if let Some(profile) = profile {
        fixture.test.hold(None, profile);
    }
    fixture.settle();
    (fixture, opened)
}

fn profile(fixture: &Fixture) -> EditorView {
    fixture.test.content::<EditorViewContent>(None).root()
}

fn show(fixture: &mut Fixture, id: Uuid, via: Option<Uuid>) {
    fixture.host.show_block(block_editor_beui::ShowRequest {
        block_id: id,
        block_type: SHOWN_TYPE,
        via,
        editor: None,
    });
    fixture.settle();
}

fn filter() -> block_editor_beui::BlockFilter {
    block_editor_beui::BlockFilter {
        name: "Block".to_owned(),
        block_types: Vec::new(),
        excluded: Vec::new(),
        templates: false,
        place: None,
    }
}

fn picking(
    beside: &Fixture,
    filter: block_editor_beui::BlockFilter,
    parent: block_editor_beui::BlockLocation,
) -> Fixture<PickerApp> {
    picking_with(beside, filter, parent, false)
}

fn picking_with(
    beside: &Fixture,
    filter: block_editor_beui::BlockFilter,
    parent: block_editor_beui::BlockLocation,
    touch: bool,
) -> Fixture<PickerApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0x3a_0003));
    host.set_account_id(beside.host.account_id());
    host.set_organization_id(beside.host.organization_id());
    let request = block_editor_beui::PickerRequest {
        filter,
        parent: BlockParent::decode(parent),
        touch,
    };
    let test =
        BeuiTest::picker_beside(&beside.test, host.clone(), request).with_size(beside.test.size());
    let mut fixture = Fixture { test, host };
    fixture.settle();
    fixture
}

fn catalog() -> block_editor_beui::Catalog {
    use block_editor_beui::be_block::BlockContent;
    let kind = |block_type: Uuid, name: &str, icon: &str| {
        let holds =
            [FolderContent::CONTENT_TYPE, WorkspaceContent::CONTENT_TYPE].contains(&block_type);
        block_editor_beui::BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: name.to_owned(),
            color: None,
            icon_codepoint: icon.to_owned(),
            children: block_plugin_api::ChildOperations {
                add: holds,
                delete: holds,
                replace: holds,
            },
            press: block_plugin_api::Press::Open,
            plain_file: false,
        }
    };
    block_editor_beui::Catalog {
        types: vec![
            kind(FolderContent::CONTENT_TYPE, "Folder", "\u{e0d7}"),
            kind(WorkspaceContent::CONTENT_TYPE, "Workspace", "\u{e2d0}"),
            kind(SHOWN_TYPE, "Text", "\u{e0cc}"),
            kind(TEMPLATE_TYPE, "Note", "\u{e185}"),
        ],
        templates: vec![block_editor_beui::TemplateDescriptor {
            editor: TEMPLATE_EDITOR.into_bytes(),
            template: "main".to_owned(),
            block_type: TEMPLATE_TYPE.into_bytes(),
            name: "Note".to_owned(),
            icon_codepoint: "\u{e185}".to_owned(),
            category: block_editor_beui::TemplateCategory::Important,
            dialog: false,
        }],
        editors: Vec::new(),
    }
}

const WIDE: Vec2 = Vec2::new(1100.0, 600.0);
const SIDEBAR_EDGE: f32 = WIDE.x * 0.22;
const SIDEBAR: block_editor_beui::beui::Pos2 =
    block_editor_beui::beui::Pos2::new(SIDEBAR_EDGE / 2.0, WIDE.y / 2.0);

fn two_blocks() -> (Fixture, Places) {
    let (fixture, _) = editor_sized(Some(WIDE));
    with_two_blocks(fixture)
}

fn with_two_blocks(mut fixture: Fixture) -> (Fixture, Places) {
    let places = places(&mut fixture);
    show(&mut fixture, places.minutes, None);
    show(&mut fixture, places.budget, Some(places.minutes));
    (fixture, places)
}

const BELOW_SIDEBAR: block_editor_beui::beui::Pos2 =
    block_editor_beui::beui::Pos2::new(SIDEBAR_EDGE / 2.0, WIDE.y - 40.0);

fn open_grip_menu(fixture: &mut Fixture, pane: &str) {
    let at = {
        let document = fixture.test.document();
        document
            .find_test_id(&format!("dock.grip.{pane}"))
            .and_then(|grip| document.node_rect(grip))
            .unwrap_or_else(|| panic!("the {pane} pane has a grip"))
            .center()
    };
    fixture
        .test
        .click_with(at, block_editor_beui::beui::PointerButton::Secondary);
    fixture.settle();
}

fn placed_in_sidebar(fixture: &Fixture, block: Uuid) -> Option<bool> {
    fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content.block_id() == Some(block.into_bytes()))
        .map(|placement| placement.rect.right() <= SIDEBAR_EDGE + 1.0)
}

const PLACES: Places = Places {
    space: Uuid::from_u128(0x3a_0101),
    trips: Uuid::from_u128(0x3a_0102),
    work: Uuid::from_u128(0x3a_0103),
    q4: Uuid::from_u128(0x3a_0104),
    minutes: Uuid::from_u128(0x3a_0105),
    budget: Uuid::from_u128(0x3a_0106),
};

struct Places {
    space: Uuid,
    trips: Uuid,
    work: Uuid,
    q4: Uuid,
    minutes: Uuid,
    budget: Uuid,
}

fn places<A: block_editor_beui::BeuiApp>(fixture: &mut Fixture<A>) -> Places {
    use block_editor_beui::BlockInfo;
    use block_editor_beui::be_block::BlockContent;
    let places = PLACES;
    let block = |id: Uuid, block_type: Uuid, parent: BlockParent, name: &str, children: &[Uuid]| {
        let mut info = BlockInfo::new(id, block_type, parent);
        info.set_name(name);
        info.references = children.to_vec();
        info
    };
    let folder = FolderContent::CONTENT_TYPE;
    let store = fixture.test.store();
    store.add_block(block(
        places.space,
        WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        "Team space",
        &[places.trips, places.work],
    ));
    store.add_block(block(
        places.trips,
        folder,
        BlockParent::Block(places.space),
        "Trips",
        &[],
    ));
    store.add_block(block(
        places.work,
        folder,
        BlockParent::Block(places.space),
        "Work",
        &[places.q4, places.minutes, places.budget],
    ));
    store.add_block(block(
        places.q4,
        folder,
        BlockParent::Block(places.work),
        "Q4",
        &[],
    ));
    store.add_block(block(
        places.minutes,
        SHOWN_TYPE,
        BlockParent::Block(places.work),
        "Minutes",
        &[],
    ));
    store.add_block(block(
        places.budget,
        SHOWN_TYPE,
        BlockParent::Block(places.work),
        "Budget",
        &[],
    ));
    fixture.settle();
    places
}

fn report_creation<A: block_editor_beui::BeuiApp>(
    fixture: &mut Fixture<A>,
    progress: block_editor_beui::CreationProgress,
) {
    report_creation_dialog(
        fixture,
        progress,
        block_editor_beui::DialogStatus::default(),
    );
}

fn report_creation_dialog<A: block_editor_beui::BeuiApp>(
    fixture: &mut Fixture<A>,
    progress: block_editor_beui::CreationProgress,
    dialog: block_editor_beui::DialogStatus,
) {
    fixture
        .test
        .report_children(|placement| block_editor_beui::ChildStatus {
            instance: block_editor_beui::EditorInstanceId(0),
            view: block_editor_beui::ViewId::MAIN,
            child: placement.child,
            available: true,
            intrinsic: None,
            aspect_ratio: None,
            active: false,
            interaction: block_editor_beui::InteractionMode::Live,
            capabilities: block_editor_beui::EditorCapabilities::default(),
            resize: block_editor_beui::ResizeMode::None,
            error: None,
            menu: Vec::new(),
            creation: matches!(
                placement.content,
                block_editor_beui::ChildContent::Creation { .. }
            )
            .then(|| progress.clone()),
            preview: Default::default(),
            dialog,
            picked: None,
            card: false,
        });
    fixture.settle();
}
