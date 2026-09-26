use block_editor_beui::be_block::LiveEdit;
use block_editor_beui::be_block::{
    Checkout, CheckoutConflict, CheckoutContent, ConflictKind, Repository, RepositoryContent,
};
use block_editor_beui::{
    BeuiApp, ConflictSide, Editor, EditorHost, VersionBranch, VersionChange, VersionChangeKind,
    VersionCommand, VersionCommit, VersionStatus,
};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::checkout::CheckoutApp;
use crate::repository::RepositoryApp;

mod a_change_is_committed_with_its_message;
mod a_conflict_is_resolved_by_taking_a_side;
mod an_empty_repository_offers_to_version_a_block;
mod checking_a_branch_out_asks_the_host;

const TEXT: Uuid = Uuid::from_u128(0x7465_7874);

fn editor<A: BeuiApp, C: LiveEdit>(content: C, status: VersionStatus) -> BeuiTest<A> {
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, Uuid::new_v4());
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, content);
    editor.set_version_status(status);
    for _ in 0..3 {
        editor.run();
    }
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
