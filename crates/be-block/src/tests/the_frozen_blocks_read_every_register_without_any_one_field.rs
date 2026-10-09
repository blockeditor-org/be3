use be_model::Model;

use crate::{Folder, TextBlock, canvas::Canvas};

#[test]
fn the_frozen_blocks_read_every_register_without_any_one_field() {
    for kind in [TextBlock::kind(), Folder::kind(), Canvas::kind()] {
        assert_eq!(kind.registers_accept_missing_fields(), Vec::<String>::new());
    }
}
