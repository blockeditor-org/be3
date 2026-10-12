use std::cell::RefCell;
use std::collections::HashMap;

use be_block::DisplaySettings;
use be_block::display_settings::DisplayMode;

use super::*;
use crate::display::{self, Screen};

mod a_display_mode_is_saved_only_once_it_is_kept;
mod a_reverted_display_mode_is_never_saved;
mod edits_written_to_the_block_cover_its_content_until_it_holds_them;

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

#[derive(Default)]
struct Store {
    copies: RefCell<HashMap<String, Vec<u8>>>,
}

impl Cache for Store {
    fn cached_copy(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.copies.borrow().get(key).cloned())
    }

    fn cache_copy(&self, key: &str, content: &[u8]) -> Result<(), String> {
        self.copies
            .borrow_mut()
            .insert(key.to_owned(), content.to_vec());
        Ok(())
    }
}

impl Store {
    fn saved(&self) -> Option<DisplaySettings> {
        let content = self
            .cached_copy(DisplaySettings::NAME)
            .expect("the copy reads")?;
        Some(
            Document::<DisplaySettings>::decode(&content)
                .expect("the saved settings decode")
                .root(),
        )
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
