use std::path::PathBuf;

use be_block::DisplaySettings;
use be_block::display_settings::DisplayMode;

use super::*;
use crate::display::{self, Screen};

mod a_display_mode_is_saved_only_once_it_is_kept;
mod a_reverted_display_mode_is_never_saved;

const MONITOR: &str = "DEL|U2723QE|1234";
const SLOW: DisplayMode = DisplayMode {
    width: 2560,
    height: 1440,
    refresh_millihertz: 59_951,
};
const FAST: DisplayMode = DisplayMode {
    width: 2560,
    height: 1440,
    refresh_millihertz: 143_998,
};

struct Store {
    path: PathBuf,
    store: AppStateStore,
}

impl Store {
    fn open() -> Self {
        let path =
            std::env::temp_dir().join(format!("block-app-settings-{}.sqlite3", Uuid::new_v4()));
        let store = AppStateStore::open(&path).expect("the app state opens");
        Self { path, store }
    }

    fn saved(&self) -> Option<DisplaySettings> {
        let content = self.store.display_settings().expect("the store reads")?;
        Some(
            Document::<DisplaySettings>::decode(&content)
                .expect("the saved settings decode")
                .root(),
        )
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn connect_monitor() {
    display::set_screens(vec![Screen {
        id: MONITOR.to_owned(),
        modes: vec![FAST, SLOW],
        default: FAST,
        preferred: SLOW,
    }]);
}

fn showing(mode: DisplayMode) -> DisplaySettings {
    let mut document = Document::new(&DisplaySettings::default());
    document.apply(&DisplaySettings::set_mode(MONITOR, Some(mode)));
    document.root()
}
