use block_editor_beui::be_block::block_url::block_url;
use uuid::Uuid;

use block_editor_beui::Waker;
use block_editor_beui::be_block::TextBlock;
use text_editor_core::{Document, TextChange};

use super::{BlockDocument, History, inside_block_url};

mod block_urls_have_no_cursor_stops_inside_them;
mod edits_go_out_as_body_changes_and_foreign_edits_land_where_they_were_made;
mod undo_and_redo_are_asked_of_the_host;
