use be_model::Model;

use crate::{Folder, Workspace};

#[test]
fn folders_and_workspaces_read_every_register_without_any_one_field() {
    for kind in [Folder::kind(), Workspace::kind()] {
        assert_eq!(kind.registers_accept_missing_fields(), Vec::<String>::new());
    }
}
