use block_editor_plugin::Editor;
use block_editor_plugin::be_block::{Repository, RepositoryContent};
use block_editor_plugin::{
    EditorHost, VersionBranch, VersionCommand, VersionCommit, VersionStatus,
};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::RepositoryApp;

mod an_empty_repository_offers_to_version_a_block;
mod checking_a_branch_out_asks_the_host;

fn harness(repository: Repository, status: VersionStatus) -> ContentHarness<RepositoryApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_version_status(status);
    let editor = Editor::new(host.clone(), Uuid::new_v4());
    let mut harness = ContentHarness::new(BeuiTest::new(editor), host);
    harness.hold(None, RepositoryContent::new(&repository));
    for _ in 0..3 {
        harness.run();
    }
    harness
}
