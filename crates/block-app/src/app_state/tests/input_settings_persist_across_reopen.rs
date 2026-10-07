use super::*;

use be_block::{InputSettings, InputSettingsContent};

#[test]
fn input_settings_persist_across_reopen() {
    let path = std::env::temp_dir().join(format!("block-app-state-{}.sqlite3", Uuid::new_v4()));
    let mut settings = InputSettingsContent::default();
    settings.apply(&InputSettings::set_keyboard_layout("de"));
    settings.apply(&InputSettings::set_natural_scroll(true));
    {
        let store = AppStateStore::open(&path).unwrap();
        assert_eq!(store.input_settings().unwrap(), None);
        store.set_input_settings(&settings.root()).unwrap();
    }
    let store = AppStateStore::open(&path).unwrap();
    assert_eq!(store.input_settings().unwrap(), Some(settings.root()));
    drop(store);
    std::fs::remove_file(path).unwrap();
}
