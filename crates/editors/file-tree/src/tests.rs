use block_editor_beui::beui::{Document, Input, Modifiers, NodeId, PointerButton, Pos2, Vec2};
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;
use workspace_ui_types::EditorViewContent;
use workspace_ui_types::{FolderContent, WorkspaceContent};

use crate::app::FileTreeApp;

mod a_block_whose_key_has_not_arrived_shows_locked_and_a_shared_workspace_can_be_left;
mod a_folder_reopened_from_the_view_is_listed_before_the_files_are_shown;
mod a_folder_still_loading_says_so_only_after_a_moment;
mod a_folder_that_could_not_be_listed_says_why;
mod a_link_to_a_deleted_block_is_marked_and_can_restore_it;
mod a_linked_block_it_cannot_see_shows_no_access_rather_than_locked;
mod a_narrow_files_pane_on_a_desktop_keeps_the_tree;
mod a_new_workspace_is_made_at_the_top_and_named_in_place;
mod a_phone_lists_what_is_shared_with_it;
mod a_phone_reaches_the_account_s_system_blocks;
mod a_phone_walks_into_folders_and_opens_files;
mod a_workspace_header_adds_into_that_workspace;
mod a_workspace_is_shown_with_what_it_holds_already_listed;
mod an_opened_folder_is_kept_open_in_the_view_and_reopens_open;
mod clicking_the_chevron_opens_and_closes_its_own_row;
mod expanding_a_folder_shows_its_children_without_more_input;
mod exporting_a_block_as_its_own_file_reads_it_from_the_host_and_saves_it;
mod exporting_any_block_as_a_block_file_asks_the_host_for_it;
mod icon_and_color_asks_the_host_from_the_row_and_the_row_shows_what_is_chosen;
mod inspecting_a_row_shows_what_is_known_about_its_block;
mod leave_is_offered_on_a_phone_but_never_to_an_administrator;
mod open_with_opens_a_block_in_the_editor_chosen_for_it;
mod renaming_a_row_edits_its_name_in_place;
mod renaming_a_row_on_a_phone_edits_it_in_place;
mod reopening_a_folder_shows_its_children_at_once;
mod swiping_back_in_a_folder_shows_the_one_above_it;
mod the_account_s_system_blocks_are_reached_from_the_bottom_of_the_files;
mod the_desktop_files_toolbar_opens_the_app_menu;
mod the_files_are_shown_by_workspace_then_what_is_shared;
mod the_folder_zoomed_into_is_kept_in_the_view_and_reopens_zoomed;
mod the_inspect_dialog_stays_on_a_narrow_screen;
mod the_phone_files_header_opens_the_app_menu;

struct Fixture {
    test: BeuiTest<FileTreeApp>,
    host: EditorHost,
}

impl Fixture {
    fn settle(&mut self) {
        self.test.run();
    }

    fn choose(&mut self, block: Uuid, label: &str) {
        self.open_menu(block);
        let document = self.test.document();
        let item = document
            .root()
            .and_then(|root| open_item(&document, root, label, false))
            .unwrap_or_else(|| panic!("right clicking a row opens a menu that offers {label}"));
        drop(document);
        self.test.click(item);
        self.settle();
    }

    fn open_menu(&mut self, block: Uuid) {
        let pos = self
            .test
            .rect_of(&format!("file-tree.{block}.row"))
            .center();
        self.test.step(vec![Input::PointerMoved(pos)]);
        self.test.step(vec![Input::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: true,
        }]);
        self.test.step(vec![Input::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: false,
        }]);
        self.settle();
    }

    fn click_item(&mut self, label: &str) {
        let document = self.test.document();
        let item = document
            .root()
            .and_then(|root| open_item(&document, root, label, false))
            .unwrap_or_else(|| panic!("an open menu offers {label}"));
        drop(document);
        self.test.click(item);
        self.settle();
    }

    fn offers(&self, label: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .and_then(|root| open_item(&document, root, label, false))
            .is_some()
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| says_within(&document, root, words))
    }

    fn opened(&mut self) -> Vec<Uuid> {
        self.test
            .take_opens()
            .into_iter()
            .map(|open| open.block_id)
            .collect()
    }
}

const MY_FILES: Uuid = Uuid::from_u128(0x006d_7966_696c_6573);

fn files_workspace(
    host: &EditorHost,
    inside: &[block_editor_beui::BlockInfo],
) -> block_editor_beui::BlockInfo {
    use block_editor_beui::be_block::BlockContent;
    use block_editor_beui::{BlockInfo, BlockParent};
    let mut files = BlockInfo::new(MY_FILES, WorkspaceContent::CONTENT_TYPE, BlockParent::Root);
    files.author = Some(host.account_id());
    files.set_name("My files");
    files
        .metadata
        .insert(&block_editor_beui::be_block::MY_FILES, &true);
    files.references = inside.iter().map(|block| block.id).collect();
    files
}

fn my_files(fixture: &mut Fixture, inside: Vec<block_editor_beui::BlockInfo>) -> Uuid {
    use block_editor_beui::BlockQuery;
    let files = files_workspace(&fixture.host, &inside);
    fixture.host.set_blocks(BlockQuery::Roots, vec![files]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), inside);
    MY_FILES
}

fn containers_hold_blocks(fixture: &mut Fixture) {
    containers(
        fixture,
        block_editor_beui::ChildOperations {
            add: true,
            delete: true,
            replace: true,
        },
    );
}

fn folders_zoom(fixture: &mut Fixture) {
    containers(fixture, block_editor_beui::ChildOperations::default());
}

fn containers(fixture: &mut Fixture, children: block_editor_beui::ChildOperations) {
    use block_editor_beui::be_block::BlockContent;
    use block_editor_beui::{BlockTypeDescriptor, Catalog, Press};
    let holding = |block_type: Uuid, name: &str| BlockTypeDescriptor {
        block_type: block_type.into_bytes(),
        display_name: name.to_owned(),
        color: None,
        icon_codepoint: String::new(),
        children,
        press: Press::Zoom,
        plain_file: false,
    };
    fixture.test.catalog(Catalog {
        types: vec![
            holding(WorkspaceContent::CONTENT_TYPE, "Workspace"),
            holding(FolderContent::CONTENT_TYPE, "Folder"),
        ],
        templates: Vec::new(),
        editors: Vec::new(),
    });
}

fn editor() -> Fixture {
    editor_sized(None)
}

fn editor_sized(size: Option<Vec2>) -> Fixture {
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), tree);
    let test = BeuiTest::new(editor);
    let test = match size {
        Some(size) => test.with_size(size),
        None => test,
    };
    let mut fixture = Fixture { test, host };
    fixture.settle();
    fixture
}

fn viewed(view: EditorViewContent) -> Fixture {
    viewed_in(view, |test| test)
}

fn viewed_on_phone(view: EditorViewContent) -> Fixture {
    viewed_in(view, |test| {
        test.with_size(Vec2::new(390.0, 800.0)).on_phone()
    })
}

fn viewed_in(
    view: EditorViewContent,
    shape: impl FnOnce(BeuiTest<FileTreeApp>) -> BeuiTest<FileTreeApp>,
) -> Fixture {
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(tree));
    let editor = OwnedEditor::new(host.clone(), tree);
    let mut fixture = Fixture {
        test: shape(BeuiTest::new(editor)),
        host,
    };
    fixture.test.hold(None, view);
    fixture.settle();
    fixture
}

fn phone() -> Fixture {
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), tree);
    let test = BeuiTest::new(editor)
        .with_size(Vec2::new(390.0, 800.0))
        .on_phone();
    let mut fixture = Fixture { test, host };
    fixture.settle();
    fixture
}

fn open_item(document: &Document, node: NodeId, label: &str, open: bool) -> Option<Pos2> {
    let open = match document.node_kind(node) {
        "overlay" => document.node_detail(node).as_deref() == Some("open"),
        _ => open,
    };
    let text = document.node_detail(node).unwrap_or_default();
    if open && document.node_kind(node) == "text" && text.trim_matches('"') == label {
        return document.node_rect(node).map(|rect| rect.center());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| open_item(&document, child, label, open))
}

fn says_within(document: &Document, node: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(node)
        .is_some_and(|text| document.text(text).contains(words))
        || document
            .children(node)
            .into_iter()
            .any(|child| says_within(&document, child, words))
}
