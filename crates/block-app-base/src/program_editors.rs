use std::sync::Mutex;

use block_plugin_api::EditorDescriptor;

static EDITORS: Mutex<(u64, Vec<EditorDescriptor>)> = Mutex::new((0, Vec::new()));

pub fn listed_since(seen: Option<u64>) -> Option<(u64, Vec<EditorDescriptor>)> {
    let held = EDITORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (seen != Some(held.0)).then(|| held.clone())
}

pub fn set(listed: Vec<EditorDescriptor>) {
    let mut held = EDITORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.1 != listed {
        held.0 += 1;
        held.1 = listed;
        crate::host::wake();
    }
}
