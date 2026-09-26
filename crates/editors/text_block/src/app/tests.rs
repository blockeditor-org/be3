use block_editor_beui::be_block::TextContent;
use block_editor_beui::be_block::block_url::block_url;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::TextApp;
use crate::app::embeds::{image_embed_directive, parse_embeds};

mod classifies_markdown_image;
mod foreign_workspace_url_is_not_an_embed;
mod image_embed_directive_uses_markdown_image;
mod image_embed_directive_uses_plain_url;
mod markdown_is_painted_with_its_styles;
mod replacing_a_referenced_block_rewrites_its_url;
mod switching_to_hex_view_shows_the_bytes;
mod the_intrinsic_size_follows_the_width_it_was_given;
mod typing_inserts_text_into_the_document;

fn editor(text: &str) -> BeuiTest<TextApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, TextContent::from(text));
    editor.run();
    editor.run();
    editor
}

fn text(editor: &BeuiTest<TextApp>) -> String {
    editor.content::<TextContent>(None).text()
}

const BLOCK_ID: Uuid = Uuid::from_u128(0xe2b8_7b59_9c69_4d75_83fd_801b_2727_1388);
const WORKSPACE_ID: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);
