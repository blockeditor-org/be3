use beui_core::clipboard::{PasteAnswer, PasteKind};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::runner::Platform;
use block_editor_plugin::EditorHost;

pub(crate) struct HostPlatform {
    host: EditorHost,
    picks: Vec<(u64, FilePickAnswer)>,
    pub(crate) show_keyboard: bool,
}

impl HostPlatform {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            host,
            picks: Vec::new(),
            show_keyboard: false,
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

    fn paste(&mut self, _kinds: &[PasteKind], answer: PasteAnswer) {
        self.host.request_paste();
        drop(answer);
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

    fn show_keyboard(&mut self) {
        self.show_keyboard = true;
    }
}
