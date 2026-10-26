use beui_core::clipboard::{PasteAnswer, PasteKind};
use beui_core::file_picker::{FilePickAnswer, FilePickRequest};
use beui_core::file_saver::{FileSaveAnswer, FileSaveRequest};
use beui_core::runner::PlatformServices;
use block_editor_plugin::accesskit::TreeUpdate;
use block_editor_plugin::{EditorHost, EditorInstanceId};

pub(crate) struct HostPlatform {
    launched: EditorHost,
    host: EditorHost,
}

impl HostPlatform {
    pub(crate) fn new(host: EditorHost) -> Self {
        Self {
            launched: host.clone(),
            host,
        }
    }

    pub(crate) fn serve(&mut self, instance: EditorInstanceId) {
        if let Some(host) = self
            .launched
            .module()
            .and_then(|module| module.host(instance))
        {
            self.host = host;
        }
    }
}

impl PlatformServices for HostPlatform {
    fn copy(&mut self, text: String) {
        self.host.copy_text(text);
    }

    fn copy_image(&mut self, png: Vec<u8>) {
        self.host.copy_image(png);
    }

    fn paste(&mut self, _kinds: &[PasteKind], answer: PasteAnswer) {
        self.host.request_paste();
        drop(answer);
    }

    fn shows_keyboard(&self) -> bool {
        true
    }

    fn delivers_paste_shortcut(&self) -> bool {
        true
    }

    fn publish_accessibility(&mut self, _tree: &mut dyn FnMut() -> TreeUpdate) {}

    fn pick_file(&mut self, request: FilePickRequest, answer: FilePickAnswer) {
        let asked = self.host.pick_file(request.filter);
        self.host.then(asked, move |pick| answer.send(pick));
    }

    fn save_file(&mut self, request: FileSaveRequest, answer: FileSaveAnswer) {
        let asked = self.host.save_file(request.file);
        self.host.then(asked, move |save| answer.send(save));
    }
}
