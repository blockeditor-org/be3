use block_editor_plugin::be_block::block_url::block_url;
use uuid::Uuid;

use block_editor_plugin::be_block::TextOp;

use super::{BlockDocument, inside_block_url};

mod block_urls_have_no_cursor_stops_inside_them;
mod undo_reverts_only_what_nobody_changed_since;
