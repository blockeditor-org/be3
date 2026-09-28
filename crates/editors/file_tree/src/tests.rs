use block_editor_beui::beui::{Document, Event, Modifiers, NodeId, PointerButton, Pos2, Vec2};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::FileTreeApp;

mod clicking_the_chevron_opens_and_closes_its_own_row;
mod expanding_a_folder_shows_its_children_without_more_input;
mod exporting_a_text_block_saves_it_as_a_markdown_file;
mod inspecting_a_row_shows_what_is_known_about_its_block;
mod the_inspect_dialog_stays_on_a_narrow_screen;

struct Fixture {
    test: BeuiTest<FileTreeApp>,
    host: EditorHost,
}

impl Fixture {
    fn settle(&mut self) {
        for _ in 0..6 {
            self.test.run();
        }
    }

    fn choose(&mut self, block: Uuid, label: &str) {
        let pos = self
            .test
            .rect_of(&format!("file-tree.{block}.row"))
            .center();
        self.test.step(vec![Event::PointerMoved(pos)]);
        self.test.step(vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.test.step(vec![Event::PointerButton {
            pos,
            button: PointerButton::Secondary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
        self.settle();
        let document = self.test.document();
        let item = document
            .root()
            .and_then(|root| open_item(document, root, label, false))
            .unwrap_or_else(|| panic!("right clicking a row opens a menu that offers {label}"));
        self.test.click_at(item);
        self.settle();
    }

    fn says(&self, words: &str) -> bool {
        let document = self.test.document();
        document
            .root()
            .is_some_and(|root| says_within(document, root, words))
    }

    fn opened(&mut self) -> Vec<Uuid> {
        self.test
            .take_opens()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    }
}

fn editor() -> Fixture {
    editor_sized(None)
}

fn editor_sized(size: Option<Vec2>) -> Fixture {
    let tree = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), tree);
    let test = BeuiTest::new(editor);
    let test = match size {
        Some(size) => test.with_size(size),
        None => test,
    };
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
        .find_map(|child| open_item(document, child, label, open))
}

fn says_within(document: &Document, node: NodeId, words: &str) -> bool {
    (document.node_kind(node) == "text" && document.text(node).contains(words))
        || document
            .children(node)
            .into_iter()
            .any(|child| says_within(document, child, words))
}
