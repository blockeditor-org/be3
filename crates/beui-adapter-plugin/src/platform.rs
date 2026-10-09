use beui_core::context::Context;
use beui_core::file_picker::{FilePickId, FilePickRequest};
use beui_core::input::{CursorIcon, ImeArea, KeyChord};
use beui_core::runner::Platform;
use block_editor_plugin::EditorHost;

pub(crate) struct HostPlatform {
    host: EditorHost,
    picks: Vec<(u64, FilePickId)>,
    pub(crate) cursor: block_plugin_api::CursorIcon,
    pub(crate) ime: Option<ImeArea>,
    pub(crate) handles_back: bool,
    pub(crate) intercepted_keys: Vec<block_plugin_api::KeyChord>,
    pub(crate) locked: bool,
}

impl HostPlatform {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            host,
            picks: Vec::new(),
            cursor: block_plugin_api::CursorIcon::Default,
            ime: None,
            handles_back: false,
            intercepted_keys: Vec::new(),
            locked: false,
        }
    }

    pub(crate) fn deliver_picks(&mut self, context: &Context) {
        let host = &self.host;
        self.picks.retain(|(request, id)| {
            let Some(pick) = host.take_pick(*request) else {
                return true;
            };
            context.file_picked(*id, beui_plugin_input::beui_file_pick(pick));
            false
        });
    }
}

impl Platform for HostPlatform {
    fn copy(&mut self, text: String) {
        self.host.copy_text(text);
    }

    fn paste(&mut self) -> Option<String> {
        self.host.request_paste();
        None
    }

    fn pick_file(&mut self, request: FilePickRequest) {
        let asked = self.host.pick_file(block_plugin_api::FileFilter {
            name: request.filter.name,
            extensions: request.filter.extensions,
            mime_types: request.filter.mime_types,
        });
        self.picks.push((asked, request.id));
    }

    fn set_cursor(&mut self, icon: CursorIcon, _touch_emulation: bool) {
        self.cursor = beui_plugin_input::protocol_cursor(icon);
    }

    fn lock_pointer(&mut self, locked: bool) {
        self.locked = locked;
    }

    fn set_handles_back(&mut self, handles: bool) {
        self.handles_back = handles;
    }

    fn set_intercepted_keys(&mut self, chords: &[KeyChord]) {
        self.intercepted_keys = chords
            .iter()
            .map(|chord| beui_plugin_input::protocol_chord(*chord))
            .collect();
    }

    fn show_ime(&mut self, ime: Option<&ImeArea>) {
        self.ime = ime.cloned();
    }
}
