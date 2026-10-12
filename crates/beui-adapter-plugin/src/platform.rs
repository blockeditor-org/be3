use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::input::{CursorIcon, ImeArea, KeyChord};
use beui_core::runner::Platform;
use block_editor_plugin::EditorHost;

pub(crate) struct HostPlatform {
    host: EditorHost,
    picks: Vec<(u64, FilePickAnswer)>,
    pub(crate) cursor: block_plugin_api::CursorIcon,
    pub(crate) ime: Option<ImeArea>,
    pub(crate) handles_back: bool,
    pub(crate) wants_keyboard: bool,
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
            wants_keyboard: false,
            intercepted_keys: Vec::new(),
            locked: false,
        }
    }

    pub(crate) fn deliver_picks(&mut self) {
        for (request, answer) in std::mem::take(&mut self.picks) {
            match self.host.take_pick(request) {
                Some(pick) => answer.send(beui_plugin_input::beui_file_pick(pick)),
                None => self.picks.push((request, answer)),
            }
        }
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

    fn delivers_paste_shortcut(&self) -> bool {
        true
    }

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        let asked = self.host.pick_file(block_plugin_api::FileFilter {
            name: request.filter.name,
            extensions: request.filter.extensions,
            mime_types: request.filter.mime_types,
        });
        self.picks.push((asked, answer));
    }

    fn set_cursor(&mut self, icon: CursorIcon, touch_emulation: bool) {
        self.cursor = match touch_emulation {
            true => block_plugin_api::CursorIcon::Crosshair,
            false => beui_plugin_input::protocol_cursor(icon),
        };
    }

    fn lock_pointer(&mut self, locked: bool) {
        self.locked = locked;
    }

    fn set_handles_back(&mut self, handles: bool) {
        self.handles_back = handles;
    }

    fn set_wants_keyboard(&mut self, wants: bool) {
        self.wants_keyboard = wants;
    }

    fn set_intercepted_keys(&mut self, chords: &[KeyChord]) {
        self.intercepted_keys = chords
            .iter()
            .filter_map(|chord| beui_plugin_input::protocol_chord(*chord))
            .collect();
    }

    fn show_ime(&mut self, ime: Option<&ImeArea>) {
        self.ime = ime.cloned();
    }
}
