use be_model::Document;
use be_model::schema::check_frozen;
use uuid::Uuid;

use super::{frozen_folder, frozen_workspace};
use crate::{Folder, Workspace};

fn bytes(hex: &str) -> Vec<u8> {
    let hex: String = hex.split_whitespace().collect();
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

#[test]
fn folders_and_workspaces_saved_before_they_were_frozen_still_load() {
    assert_eq!(
        check_frozen::<Folder>(frozen_folder::REGISTRY, frozen_folder::DOCUMENT),
        Ok(())
    );
    let folder = Document::<Folder>::from_bytes(&bytes(frozen_folder::DOCUMENT)).unwrap();
    assert_eq!(
        folder.root().blocks(),
        [
            Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_0001),
            Uuid::from_u128(0xd4e3_67ab_db73_4419_bffc_b913_1288_0002),
            Uuid::from_u128(0x5c1a_0000_4000_8000_0000_0000_0000_0003),
        ]
    );
    assert_eq!(folder.block_refs().len(), 3);

    assert_eq!(
        check_frozen::<Workspace>(frozen_workspace::REGISTRY, frozen_workspace::DOCUMENT),
        Ok(())
    );
    let workspace = Document::<Workspace>::from_bytes(&bytes(frozen_workspace::DOCUMENT)).unwrap();
    assert_eq!(
        workspace.root().blocks(),
        [
            Uuid::from_u128(0x6f4d_8f85_7991_4cdf_ae41_b526_30df_0001),
            Uuid::from_u128(0xd4e3_67ab_db73_4419_bffc_b913_1288_0002),
        ]
    );
    assert_eq!(workspace.block_refs().len(), 2);
}
