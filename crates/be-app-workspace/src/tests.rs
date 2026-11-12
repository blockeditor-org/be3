use be_sdk_beui::be_block::{EditorView, EditorViewContent, ViewState};
use be_type_workspace::{FolderContent, WorkspaceContent};

use be_sdk_beui::beui::{NodeId, Rect, ScrollDelta, Vec2};

use be_sdk_beui::beui::native::Document;
use be_sdk_beui::{
    BlockParent, ChildContent, ChildId, EditorHost, HostWindowId, OwnedEditor, Picture,
    WindowsWatch,
};
use be_sdk_test::BeuiTest;
use std::collections::HashMap;
use uuid::Uuid;

use crate::picker::PickerRoot;
use crate::rename::RenameRoot;
use crate::root::WorkspaceUiRoot;

mod a_block_in_a_tab_points_its_dialogs_at_the_tab;
mod a_block_opened_while_another_is_shown_gets_its_own_tab;
mod a_block_opens_in_the_editor_it_keeps_for_itself;
mod a_block_tab_asks_its_editor_for_the_top_bar;
mod a_block_tab_dragged_onto_the_sidebar_stays_with_the_blocks;
mod a_block_tab_unlocked_from_its_grip_moves_into_the_sidebar;
mod a_block_tabs_menu_mutes_its_notifications;
mod a_block_tabs_menu_previews_its_access_and_comes_back;
mod a_closed_tab_gives_up_its_view_block;
mod a_deleted_block_says_so_above_it_and_can_be_restored;
mod a_dialog_of_a_programs_dialog_points_at_the_dialog_it_came_from;
mod a_failed_creation_is_shown_and_dismissing_it_cancels_the_pick;
mod a_file_with_no_editor_of_its_own_opens_in_the_program_for_its_extension;
mod a_folder_chosen_on_the_location_page_holds_the_block_made_there;
mod a_host_panel_the_host_asks_for_is_placed_in_its_own_window;
mod a_modal_window_sends_presses_on_its_parent_to_itself;
mod a_new_file_beside_a_workspace_on_a_phone_goes_to_unfiled;
mod a_phone_shows_one_file_at_a_time_and_the_dock_bar_goes_back_or_switches;
mod a_pick_into_unfiled_says_it_makes_the_block_there;
mod a_pick_made_with_no_place_commits_its_block_to_unfiled;
mod a_presentation_offers_its_slide_templates_first;
mod a_profile_keeps_its_tabs_as_view_blocks_and_reopens_them;
mod a_programs_dialog_that_resizes_itself_still_points_at_its_tab;
mod a_programs_modal_dialog_points_at_its_tab_and_dims_its_window;
mod a_reopened_phone_goes_back_through_the_files_in_the_order_they_were_shown;
mod a_reopened_tab_shows_its_saved_title_before_its_block_is_known;
mod a_reopened_workspace_with_no_file_open_says_so;
mod a_shown_block_is_remembered_in_the_recents;
mod a_shown_block_is_reported_as_focused;
mod a_template_copied_with_no_place_is_made_in_unfiled;
mod a_template_of_the_workspace_is_copied_into_place;
mod a_template_without_a_dialog_is_created_and_answers_the_pick;
mod a_window_tab_is_locked_to_the_blocks_area_like_a_block_tab;
mod a_window_tab_never_shown_is_named_and_shown_when_its_program_raises_it;
mod a_window_tab_shows_the_icon_its_program_set;
mod a_window_the_host_makes_fullscreen_covers_the_workspace_until_it_leaves;
mod a_window_the_host_runs_gets_a_tab_until_its_program_closes_it;
mod a_window_with_a_parent_hangs_from_its_tab_and_hides_with_it;
mod a_workspace_opens_on_its_landing_page;
mod an_import_into_a_folder_is_made_inside_it;
mod an_import_keeps_the_location_it_started_with;
mod an_import_with_no_place_is_made_in_unfiled;
mod an_open_menu_is_withheld_from_the_block_under_it;
mod an_unlocked_tab_moved_into_the_sidebar_is_still_there_when_the_workspace_reopens;
mod an_unreadable_layout_is_kept_until_the_person_resets_it;
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
mod on_a_phone_tapping_a_files_name_runs_its_rename;
mod on_a_phone_the_menu_and_its_location_page_rise_as_a_sheet;
mod on_a_phone_the_more_sheet_keeps_its_cancel_below_the_list;
mod on_a_wide_phone_the_more_sheet_fills_the_screen_width;
mod resetting_an_unreadable_layout_starts_afresh_and_saves_it;
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
mod the_editors_pane_with_no_file_open_keeps_its_grip;
mod the_link_card_shows_where_the_chosen_block_lives_and_links_it;
mod the_location_page_shows_the_folders_as_a_tree_with_the_chosen_one_checked;
mod the_picker_never_offers_a_deleted_block_to_link;
mod the_rename_card_sets_a_blocks_name_icon_and_color;
mod the_rename_form_opens_with_its_name_selected;
mod the_rename_form_saves_a_typed_name_and_closes_on_enter;
mod the_share_dialog_lists_again_when_a_contact_is_made;
mod the_share_dialog_says_where_each_persons_access_comes_from;
mod widening_the_phone_with_its_switcher_open_keeps_the_workspace;

const SHOWN_TYPE: Uuid = Uuid::from_u128(0x7368_6f77_6e2d_7479_7065_2d74_6573_7431);
const TEMPLATE_EDITOR: Uuid = Uuid::from_u128(0x7465_6d70_6c61_7465_2d65_6469_746f_7231);
const TEMPLATE_TYPE: Uuid = Uuid::from_u128(0x7465_6d70_6c61_7465_2d74_7970_6531_0000);

struct Fixture<A: be_sdk_beui::BeuiRoot = WorkspaceUiRoot> {
    test: BeuiTest<A>,
    host: EditorHost,
    windows: Vec<TestWindow>,
    reported: HashMap<ChildId, TestWindow>,
}

#[derive(Clone, Debug, PartialEq)]
struct TestWindow {
    id: u64,
    title: String,
    name: String,
    program_id: String,
    parent: Option<u64>,
    modal: bool,
    size: Vec2,
    fullscreen: bool,
    icon: Option<Picture>,
    raised: u64,
}

impl TestWindow {
    fn named(&self, parent: Option<&TestWindow>) -> String {
        let name = match self.name.is_empty() {
            true => parent.map_or_else(String::new, |parent| parent.name.clone()),
            false => self.name.clone(),
        };
        let title = self.title.trim();
        let title = title
            .strip_suffix(&format!(" - {name}"))
            .filter(|_| !name.is_empty())
            .unwrap_or(title);
        let untitled = title.is_empty() || title == name || title == self.program_id;
        match (name.is_empty(), untitled) {
            (true, _) => title.to_owned(),
            (false, true) => name,
            (false, false) => format!("{name} \u{2014} {title}"),
        }
    }

    fn listed(&self) -> be_sdk_beui::HostWindow {
        be_sdk_beui::HostWindow {
            id: HostWindowId(self.id),
            kind: be_sdk_beui::WindowKind::Window,
            parent: self.parent.map(HostWindowId),
            modal: self.modal,
            program_id: self.program_id.clone(),
            origin: Default::default(),
            size: (self.size.x >= 1.0 && self.size.y >= 1.0).then_some(self.size),
            maximized: false,
        }
    }
}

impl<A: be_sdk_beui::BeuiRoot> Fixture<A> {
    fn new(test: BeuiTest<A>, host: EditorHost) -> Self {
        Self {
            test,
            host,
            windows: Vec::new(),
            reported: HashMap::new(),
        }
    }

    fn settle(&mut self) {
        self.test.run();
        for _ in 0..4 {
            if !self.report_windows() {
                break;
            }
            self.test.run();
        }
    }

    fn show_windows(&mut self, windows: &[TestWindow]) {
        self.windows = windows.to_vec();
        let listed: Vec<be_sdk_beui::HostWindow> = windows.iter().map(TestWindow::listed).collect();
        self.host.set_watched::<WindowsWatch>(&listed);
        self.settle();
    }

    fn report_windows(&mut self) -> bool {
        let placed: Vec<(ChildId, u64)> = self
            .test
            .every_child()
            .iter()
            .filter_map(|placement| match placement.content {
                ChildContent::Window(window) => Some((placement.child, window.0)),
                _ => None,
            })
            .collect();
        let mut reported = false;
        for (child, id) in placed {
            let Some(window) = self.windows.iter().find(|held| held.id == id).cloned() else {
                continue;
            };
            if self.reported.get(&child) == Some(&window) {
                continue;
            }
            self.reported.insert(child, window.clone());
            let parent = window
                .parent
                .and_then(|parent| self.windows.iter().find(|held| held.id == parent));
            let title = window.named(parent);
            self.test.report_child(child, move |status| {
                status.title = Some(title);
                status.icon = window.icon;
                status.group = Some(window.program_id).filter(|id| !id.is_empty());
                status.raised = window.raised;
                status.fullscreen = window.fullscreen;
                status.intrinsic = Some(be_sdk_beui::Size {
                    width: window.size.x,
                    height: window.size.y,
                });
            });
            reported = true;
        }
        reported
    }

    fn focused_windows(&mut self) -> Vec<u64> {
        let given = self.test.take_child_focus();
        given
            .into_iter()
            .filter_map(|child| self.reported.get(&child).map(|window| window.id))
            .collect()
    }

    fn closed_windows(&mut self) -> Vec<u64> {
        let closes = self.test.take_child_closes();
        closes
            .into_iter()
            .filter_map(|child| self.reported.get(&child).map(|window| window.id))
            .collect()
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

    fn drag_text(&mut self, words: &str, to: be_sdk_beui::beui::Pos2) {
        let from = self.text_rect(words).center();
        self.test.drag(from, to);
        self.settle();
        self.test.settle();
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
            be_sdk_beui::beui::Input::PointerMoved(at),
            be_sdk_beui::beui::Input::Scroll(ScrollDelta::Pixels(Vec2::new(-200.0, 0.0))),
        ]);
        self.test.step(vec![be_sdk_beui::beui::Input::ScrollEnded]);
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
        let mut closes: Vec<String> = self
            .test
            .test_ids()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| id.starts_with("dock.tab.") && id.ends_with(".close"))
            .map(str::to_owned)
            .collect();
        closes.sort_by(|a, b| (a.len(), a).cmp(&(b.len(), b)));
        closes
            .iter()
            .filter_map(|id| document.find_test_id(id))
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

const PROGRAMS: [(&str, &str); 5] = [
    ("org.gnome.TextEditor", "Text Editor"),
    ("org.gnome.Terminal", "Terminal"),
    ("com.github.PintaProject.Pinta", "Pinta"),
    ("org.mozilla.firefox", "Firefox"),
    ("org.gnome.Totem", "Videos"),
];

fn window(id: u64, title: &str, parent: Option<u64>) -> TestWindow {
    let (program_id, name) = match parent {
        Some(_) => ("", ""),
        None => PROGRAMS
            .into_iter()
            .find(|(_, name)| title == *name || title.ends_with(&format!(" - {name}")))
            .unwrap_or(("", "")),
    };
    TestWindow {
        id,
        title: title.to_owned(),
        name: name.to_owned(),
        program_id: program_id.to_owned(),
        parent,
        modal: false,
        size: Vec2::new(320.0, 200.0),
        fullscreen: false,
        icon: None,
        raised: 0,
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
    let test = match size {
        Some(size) => BeuiTest::new(editor).with_size(size),
        None => BeuiTest::new(editor),
    };
    let mut fixture = Fixture::new(test, host);
    if let Some(profile) = profile {
        fixture.test.hold(None, profile);
    }
    fixture.settle();
    (fixture, opened)
}

fn unreadable_layout() -> ViewState {
    ViewState {
        bytes: vec![0xff; 6],
        refs: vec![Uuid::from_u128(0x3a_00f1)],
    }
}

fn profile(fixture: &Fixture) -> EditorView {
    fixture.test.content::<EditorViewContent>(None).root()
}

fn show(fixture: &mut Fixture, id: Uuid, via: Option<Uuid>) {
    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: id,
        block_type: SHOWN_TYPE,
        via,
        editor: None,
    });
    fixture.settle();
}

fn filter() -> be_sdk_beui::BlockFilter {
    be_sdk_beui::BlockFilter {
        name: "Block".to_owned(),
        block_types: Vec::new(),
        excluded: Vec::new(),
        templates: false,
        place: None,
    }
}

fn picking(
    beside: &Fixture,
    filter: be_sdk_beui::BlockFilter,
    parent: be_sdk_beui::BlockLocation,
) -> Fixture<PickerRoot> {
    picking_with(beside, filter, parent, false)
}

fn picking_with(
    beside: &Fixture,
    filter: be_sdk_beui::BlockFilter,
    parent: be_sdk_beui::BlockLocation,
    touch: bool,
) -> Fixture<PickerRoot> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0x3a_0003));
    host.set_account_id(beside.host.account_id());
    host.set_organization_id(beside.host.organization_id());
    let request = be_sdk_beui::PickerRequest {
        filter,
        parent: BlockParent::decode(parent),
        sheet: touch,
    };
    let test =
        BeuiTest::picker_beside(&beside.test, host.clone(), request).with_size(beside.test.size());
    let mut fixture = Fixture::new(test, host);
    fixture.settle();
    fixture
}

fn renaming(beside: &Fixture, block: Uuid) -> Fixture<RenameRoot> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::from_u128(0x3a_0004));
    host.set_account_id(beside.host.account_id());
    host.set_organization_id(beside.host.organization_id());
    let test = BeuiTest::rename_beside(&beside.test, host.clone(), block)
        .with_size(Vec2::new(392.0, 560.0));
    let mut fixture = Fixture::new(test, host);
    fixture.settle();
    fixture
}

fn catalog() -> be_sdk_beui::Catalog {
    use be_sdk_beui::be_block::BlockContent;
    let kind = |block_type: Uuid, name: &str, icon: &str| {
        let holds =
            [FolderContent::CONTENT_TYPE, WorkspaceContent::CONTENT_TYPE].contains(&block_type);
        be_sdk_beui::BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: name.to_owned(),
            color: None,
            icon_codepoint: icon.to_owned(),
            children: be_sdk_api::ChildOperations {
                add: holds,
                delete: holds,
                replace: holds,
                reorder: holds,
            },
            press: be_sdk_api::Press::Open,
            plain_file: false,
        }
    };
    be_sdk_beui::Catalog {
        types: vec![
            kind(FolderContent::CONTENT_TYPE, "Folder", "\u{e0d7}"),
            kind(WorkspaceContent::CONTENT_TYPE, "Workspace", "\u{e2d0}"),
            kind(SHOWN_TYPE, "Text", "\u{e0cc}"),
            kind(TEMPLATE_TYPE, "Note", "\u{e185}"),
        ],
        templates: vec![be_sdk_beui::TemplateDescriptor {
            editor: TEMPLATE_EDITOR.into_bytes(),
            template: "main".to_owned(),
            block_type: TEMPLATE_TYPE.into_bytes(),
            name: "Note".to_owned(),
            icon_codepoint: "\u{e185}".to_owned(),
            category: be_sdk_beui::TemplateCategory::Important,
            dialog: false,
        }],
        editors: Vec::new(),
    }
}

const WIDE: Vec2 = Vec2::new(1100.0, 600.0);
const SIDEBAR_EDGE: f32 = crate::root::FILES_WIDTH;
const SIDEBAR: be_sdk_beui::beui::Pos2 =
    be_sdk_beui::beui::Pos2::new(SIDEBAR_EDGE / 2.0, WIDE.y / 2.0);

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

const BELOW_SIDEBAR: be_sdk_beui::beui::Pos2 =
    be_sdk_beui::beui::Pos2::new(SIDEBAR_EDGE / 2.0, WIDE.y - 40.0);

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
        .click_with(at, be_sdk_beui::beui::PointerButton::Secondary);
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

fn places<A: be_sdk_beui::BeuiRoot>(fixture: &mut Fixture<A>) -> Places {
    use be_sdk_beui::BlockInfo;
    use be_sdk_beui::be_block::BlockContent;
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

fn report_creation<A: be_sdk_beui::BeuiRoot>(
    fixture: &mut Fixture<A>,
    progress: be_sdk_beui::CreationProgress,
) {
    report_creation_dialog(fixture, progress, be_sdk_beui::DialogStatus::default());
}

fn report_creation_dialog<A: be_sdk_beui::BeuiRoot>(
    fixture: &mut Fixture<A>,
    progress: be_sdk_beui::CreationProgress,
    dialog: be_sdk_beui::DialogStatus,
) {
    fixture
        .test
        .report_children(|placement| be_sdk_beui::ChildStatus {
            instance: be_sdk_beui::EditorInstanceId(0),
            view: be_sdk_beui::ViewId::MAIN,
            child: placement.child,
            available: true,
            intrinsic: None,
            active: false,
            interaction: be_sdk_beui::InteractionMode::Live,
            capabilities: be_sdk_beui::EditorCapabilities::default(),
            resize: be_sdk_beui::ResizeMode::None,
            error: None,
            menu: Vec::new(),
            creation: matches!(
                placement.content,
                be_sdk_beui::ChildContent::Creation { .. }
            )
            .then(|| progress.clone()),
            preview: Default::default(),
            dialog,
            picked: None,
            card: false,
            title: None,
            icon: None,
            group: None,
            raised: 0,
            rung: 0,
            minimized: 0,
            fullscreen: false,
            notice: None,
            menu_asked: None,
        });
    fixture.settle();
}
