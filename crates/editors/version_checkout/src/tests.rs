use block_editor_plugin::be_block::{Checkout, CheckoutConflict, CheckoutContent, ConflictKind};
use block_editor_plugin::{
    ConflictSide, Editor, EditorHost, VersionBranch, VersionChange, VersionChangeKind,
    VersionCommand, VersionStatus,
};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::CheckoutApp;

mod a_change_is_committed_with_its_message;
mod a_conflict_is_resolved_by_taking_a_side;

const TEXT: Uuid = Uuid::from_u128(0x7465_7874);

fn harness(checkout: Checkout, status: VersionStatus) -> ContentHarness<CheckoutApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_version_status(status);
    let editor = Editor::new(host.clone(), Uuid::new_v4());
    let mut harness = ContentHarness::new(BeuiTest::new(editor), host);
    harness.hold(None, CheckoutContent::new(&checkout));
    for _ in 0..3 {
        harness.run();
    }
    harness
}

fn branches() -> Vec<VersionBranch> {
    vec![VersionBranch {
        name: "main".into(),
        head: [7; 32],
    }]
}
