use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::Scene3DApp;

mod clicking_the_scene_grabs_the_cursor_and_escape_releases_it;

const ACCOUNT: Uuid = Uuid::from_u128(0x3364_2d74_6573_742d_6163_636f_756e_7401);

fn editor() -> (BeuiTest<Scene3DApp>, EditorHost) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(ACCOUNT);
    let editor = Editor::new(host.clone(), block);
    (BeuiTest::new(editor), host)
}
