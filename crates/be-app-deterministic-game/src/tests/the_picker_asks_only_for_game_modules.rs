use super::*;

#[test]
fn the_picker_asks_only_for_game_modules() {
    let filter = module_filter();

    assert_eq!(
        filter.block_types,
        [GameModuleContent::CONTENT_TYPE.into_bytes()]
    );
    assert!(!filter.templates);
}
