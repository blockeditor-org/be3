use be_model::Model;

use crate::database::Database;
use crate::database_schema::DatabaseSchema;
use crate::database_view::DatabaseView;
use crate::{Folder, TextBlock, Workspace, canvas::Canvas};

#[test]
fn the_frozen_blocks_read_every_register_without_any_one_field() {
    for kind in [
        TextBlock::kind(),
        Folder::kind(),
        Workspace::kind(),
        Canvas::kind(),
        Database::kind(),
        DatabaseSchema::kind(),
        DatabaseView::kind(),
    ] {
        assert_eq!(kind.registers_accept_missing_fields(), Vec::<String>::new());
    }
}
