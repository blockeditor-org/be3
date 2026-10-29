use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR};
use block_editor_beui::beui::icons::{ICON_FILE_TEXT, ICON_LOCK, ICON_STAR, ICON_TRASH};

use super::*;

#[test]
fn icon_overrides_keep_lock_and_deleted_markers() {
    let client = client();
    let types = BlockCatalog::new([(
        LISTED,
        BlockTypeEntry {
            display_name: "Note".to_owned(),
            color: Some([65, 145, 210]),
            icon: Some(ICON_FILE_TEXT),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        },
    )]);
    let describe = |info: &BlockInfo| {
        super::super::describe(
            &client,
            &types,
            &block_types(),
            info,
            Placement {
                key: RowKey::Block(vec![info.id]),
                depth: 1,
                container: None,
                expanded: false,
                trashed: false,
            },
            |id| id.to_string(),
        )
    };
    let mut info = BlockInfo::new(Uuid::from_u128(5), LISTED, BlockParent::Root);
    let row = describe(&info);
    assert_eq!(row.glyph, Some(ICON_FILE_TEXT));
    assert_eq!(row.icon_color, Some([65, 145, 210]));
    info.metadata.insert(&ICON, &"star".to_owned());
    info.metadata.insert(&ICON_COLOR, &[203, 91, 125]);
    let row = describe(&info);
    assert_eq!(row.glyph, Some(ICON_STAR));
    assert_eq!(row.icon_color, Some([203, 91, 125]));
    info.deleted = true;
    let row = describe(&info);
    assert_eq!(row.glyph, Some(ICON_TRASH));
    assert_eq!(row.icon_color, None);
    info.locked = true;
    let row = describe(&info);
    assert_eq!(row.glyph, Some(ICON_LOCK));
    assert_eq!(row.icon_color, None);
}
