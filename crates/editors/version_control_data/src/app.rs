use std::time::{SystemTime, UNIX_EPOCH};

use block_client::blocks::version_control_data::{CommitId, VersionControlData};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

pub struct VersionControlDataApp;

impl block_editor_plugin::BeuiApp for VersionControlDataApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <RepositoryView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        let client = creation.client();
        let author = client.account_id();
        let data = VersionControlData::new(author, unix_seconds_now());
        Ok(client.create_block(data).id())
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn short_author(author: Uuid) -> String {
    author
        .simple()
        .to_string()
        .chars()
        .take(CommitId::SHORT_LEN)
        .collect()
}
