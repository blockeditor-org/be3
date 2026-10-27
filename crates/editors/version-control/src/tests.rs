use block_editor_beui::be_block::LiveEdit;
use block_editor_beui::be_block::{
    Checkout, CheckoutConflict, CheckoutContent, ConflictKind, Repository, RepositoryContent,
};
use block_editor_beui::beui::Vec2;
use block_editor_beui::{
    BeuiApp, ConflictSide, EditorHost, OwnedEditor, VersionBranch, VersionChange,
    VersionChangeKind, VersionCommand, VersionCommit, VersionOperation, VersionStatus, short_id,
};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use crate::checkout::CheckoutApp;
use crate::repository::RepositoryApp;

mod a_change_is_saved_with_its_description;
mod a_change_with_clashing_edits_can_still_be_saved;
mod a_checkout_waits_for_its_version_status;
mod a_conflict_is_resolved_by_taking_a_side;
mod a_step_is_undone_from_what_was_done;
mod an_empty_repository_offers_to_version_a_block;
mod another_branch_is_combined_into_this_one;
mod checking_a_branch_out_asks_the_host;

const TEXT: Uuid = Uuid::from_u128(0x7465_7874);

fn editor<A: BeuiApp, C: LiveEdit + block_editor_beui::be_block::Merge + Clone + Default>(
    content: C,
    status: VersionStatus,
) -> BeuiTest<A> {
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host, Uuid::new_v4());
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, content);
    editor.set_version_status(status);
    editor.run();
    editor
}

fn repository(repository: Repository, status: VersionStatus) -> BeuiTest<RepositoryApp> {
    editor(RepositoryContent::new(&repository), status)
}

fn checkout(checkout: Checkout, status: VersionStatus) -> BeuiTest<CheckoutApp> {
    editor(CheckoutContent::new(&checkout), status)
}

fn branches() -> Vec<VersionBranch> {
    vec![VersionBranch {
        name: "main".into(),
        head: [7; 32],
    }]
}

fn own(editor: &BeuiTest<impl BeuiApp>) -> Uuid {
    editor.block_id().expect("the editor has a block")
}

fn history() -> Vec<VersionCommit> {
    vec![VersionCommit {
        id: [7; 32],
        change: [3; 16],
        parents: Vec::new(),
        author: [1; 16],
        time: 1_000,
        message: "Started keeping versions".into(),
        conflicted: false,
    }]
}

fn working() -> VersionCommit {
    VersionCommit {
        id: [9; 32],
        change: [0xab; 16],
        parents: vec![[7; 32]],
        author: [1; 16],
        time: 2_000,
        message: String::new(),
        conflicted: false,
    }
}
