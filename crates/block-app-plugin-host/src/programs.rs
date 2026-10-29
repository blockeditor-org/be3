use std::sync::Mutex;

use block_plugin_api::EditorDescriptor;

static EDITORS: Mutex<(u64, Vec<EditorDescriptor>)> = Mutex::new((0, Vec::new()));

pub fn program_editors() -> (u64, Vec<EditorDescriptor>) {
    EDITORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub fn set_program_editors(listed: Vec<EditorDescriptor>) {
    let mut held = EDITORS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.1 != listed {
        held.0 += 1;
        held.1 = listed;
        block_app_base::host::wake();
    }
}
